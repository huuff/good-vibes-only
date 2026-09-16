use crate::git::output;
use anyhow::{Context, Result, bail};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

pub struct Package {
    pub name: String,
    pub directory: Option<String>,
    pub manifest_path: Option<String>,
    pub dependencies: BTreeSet<String>,
    pub member: bool,
    pub metadata: Value,
    pub manifest: Option<toml::Value>,
    pub lints: Option<toml::Value>,
    pub features: BTreeSet<String>,
}

pub struct Rule {
    pub paths: GlobSet,
    pub packages: Vec<String>,
}

pub struct Snapshot {
    pub packages: BTreeMap<String, Package>,
    pub manifest_path: String,
    pub lock_path: String,
    pub global: toml::Value,
    pub rules_config: Option<toml::Value>,
    pub rules: Vec<Rule>,
}

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Value>,
    workspace_root: String,
    workspace_members: BTreeSet<String>,
    resolve: Option<Resolve>,
}
#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}
#[derive(Deserialize)]
struct Node {
    id: String,
    dependencies: Vec<String>,
    features: BTreeSet<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    #[serde(default)]
    rules: Vec<RuleConfig>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleConfig {
    paths: Vec<String>,
    packages: Vec<String>,
}

impl Snapshot {
    pub fn load(root: &Path, manifest: &Path, offline: bool) -> Result<Self> {
        let root = root.canonicalize()?;
        let mut command = Command::new("cargo");
        command
            .current_dir(
                root.join(manifest)
                    .parent()
                    .context("manifest has no parent")?,
            )
            .args([
                "metadata",
                "--format-version",
                "1",
                "--all-features",
                "--locked",
                "--manifest-path",
            ])
            .arg(root.join(manifest));
        if offline {
            command.arg("--offline");
        }
        let metadata: Metadata = serde_json::from_slice(&output(&mut command).context(
            "reading Cargo metadata for committed snapshot (commit an up-to-date Cargo.lock)",
        )?)?;
        let workspace = Path::new(&metadata.workspace_root);
        let manifest_path = relative(&root, &workspace.join("Cargo.toml"))?;
        let lock_path = relative(&root, &workspace.join("Cargo.lock"))?;
        let workspace_manifest = read_manifest(&workspace.join("Cargo.toml"))?;
        let global = global_settings(&workspace_manifest);
        let rules_config = workspace_manifest
            .get("workspace")
            .and_then(|v| v.get("metadata"))
            .and_then(|v| v.get("affected"))
            .cloned();
        let mut ids = BTreeMap::new();
        for p in &metadata.packages {
            let id = string(p, "id")?;
            let key = if p["source"].is_null() {
                format!(
                    "path:{}",
                    relative(&root, Path::new(string(p, "manifest_path")?))?
                )
            } else {
                id.to_owned()
            };
            ids.insert(id.to_owned(), key);
        }
        let mut packages = BTreeMap::new();
        let mut declared = BTreeMap::new();
        for p in metadata.packages {
            let id = string(&p, "id")?;
            let key = ids[id].clone();
            let local_path = p["source"]
                .is_null()
                .then(|| Path::new(string(&p, "manifest_path").expect("validated manifest path")));
            let directory = local_path
                .map(|p| relative(&root, p.parent().expect("manifest parent")))
                .transpose()?;
            let package_manifest_path = local_path.map(|p| relative(&root, p)).transpose()?;
            let mut manifest = local_path.map(read_manifest).transpose()?;
            let lints = match (local_path, manifest.as_ref()) {
                (Some(path), Some(manifest)) => inherited_lints(&root, path, manifest)?,
                _ => None,
            };
            if let Some(manifest) = manifest.as_mut().and_then(toml::Value::as_table_mut) {
                // Only the analyzed root is compared separately. An excluded
                // path dependency may have its own workspace settings/lints.
                if package_manifest_path.as_ref() == Some(&manifest_path) {
                    for field in ["workspace", "profile", "patch", "replace"] {
                        manifest.remove(field);
                    }
                }
            }
            let dependencies = p["dependencies"]
                .as_array()
                .context("Cargo metadata missing dependencies")?
                .iter()
                .filter_map(|d| d["path"].as_str())
                .map(|p| {
                    relative(&root, &Path::new(p).join("Cargo.toml")).map(|p| format!("path:{p}"))
                })
                .collect::<Result<BTreeSet<_>>>()?;
            declared.insert(key.clone(), dependencies);
            let mut package_metadata = p.clone();
            // Cargo IDs are opaque and may URL-encode the temporary directory.
            // Use the stable identity we assigned, rather than text substitution.
            package_metadata["id"] = Value::String(key.clone());
            packages.insert(
                key,
                Package {
                    name: string(&p, "name")?.into(),
                    directory,
                    manifest_path: package_manifest_path,
                    dependencies: BTreeSet::new(),
                    member: metadata.workspace_members.contains(id),
                    metadata: normalize(package_metadata, &root),
                    manifest,
                    lints,
                    features: BTreeSet::new(),
                },
            );
        }
        for node in metadata
            .resolve
            .context("Cargo returned no resolved graph")?
            .nodes
        {
            let package = packages
                .get_mut(&ids[&node.id])
                .context("resolved package missing")?;
            package.dependencies = node
                .dependencies
                .into_iter()
                .map(|id| ids[&id].clone())
                .collect();
            package.features = node.features;
        }
        for (id, dependencies) in declared {
            for dependency in &dependencies {
                if !packages.contains_key(dependency) {
                    bail!(
                        "declared local dependency {dependency} was not resolved by Cargo; cannot safely determine affected packages"
                    );
                }
            }
            packages
                .get_mut(&id)
                .expect("known package")
                .dependencies
                .extend(dependencies);
        }
        let rules = parse_rules(rules_config.clone(), &packages)?;
        Ok(Self {
            packages,
            manifest_path,
            lock_path,
            global,
            rules_config,
            rules,
        })
    }

    pub fn empty_like(head: &Self) -> Self {
        Self {
            packages: BTreeMap::new(),
            manifest_path: head.manifest_path.clone(),
            lock_path: head.lock_path.clone(),
            global: toml::Value::Table(Default::default()),
            rules_config: None,
            rules: Vec::new(),
        }
    }
}

fn parse_rules(
    config: Option<toml::Value>,
    packages: &BTreeMap<String, Package>,
) -> Result<Vec<Rule>> {
    let Some(config) = config else {
        return Ok(Vec::new());
    };
    let config: Configuration = config
        .try_into()
        .context("invalid workspace.metadata.affected configuration")?;
    config
        .rules
        .into_iter()
        .map(|rule| {
            if rule.paths.is_empty() {
                bail!("affected rule paths must not be empty");
            }
            for name in &rule.packages {
                if name != "*" && !packages.values().any(|p| p.member && &p.name == name) {
                    bail!("affected rule names unknown workspace package {name:?}");
                }
            }
            let mut builder = GlobSetBuilder::new();
            for path in &rule.paths {
                builder.add(
                    Glob::new(path).with_context(|| format!("invalid affected glob {path:?}"))?,
                );
            }
            Ok(Rule {
                paths: builder.build()?,
                packages: rule.packages,
            })
        })
        .collect()
}

fn inherited_lints(
    root: &Path,
    path: &Path,
    manifest: &toml::Value,
) -> Result<Option<toml::Value>> {
    if manifest
        .get("lints")
        .and_then(|v| v.get("workspace"))
        .and_then(toml::Value::as_bool)
        != Some(true)
    {
        return Ok(None);
    }
    let directory = path.parent().context("package manifest has no parent")?;
    if let Some(explicit) = manifest
        .get("package")
        .and_then(|v| v.get("workspace"))
        .and_then(toml::Value::as_str)
    {
        let workspace = directory.join(explicit).join("Cargo.toml").canonicalize()?;
        relative(root, &workspace)?;
        return Ok(read_manifest(&workspace)?
            .get("workspace")
            .and_then(|v| v.get("lints"))
            .cloned());
    }
    for directory in directory.ancestors().take_while(|p| p.starts_with(root)) {
        let candidate = directory.join("Cargo.toml");
        if !candidate.is_file() {
            continue;
        }
        let candidate = read_manifest(&candidate)?;
        if let Some(workspace) = candidate.get("workspace") {
            return Ok(workspace.get("lints").cloned());
        }
    }
    bail!(
        "could not locate inherited workspace lints for {}",
        path.display()
    )
}

fn read_manifest(path: &Path) -> Result<toml::Value> {
    fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?
        .parse()
        .with_context(|| format!("parsing {}", path.display()))
}
fn global_settings(manifest: &toml::Value) -> toml::Value {
    let mut table = manifest.as_table().cloned().unwrap_or_default();
    table.retain(|key, _| ["workspace", "profile", "patch", "replace"].contains(&key));
    if let Some(workspace) = table
        .get_mut("workspace")
        .and_then(toml::Value::as_table_mut)
    {
        for key in [
            "members",
            "default-members",
            "exclude",
            "dependencies",
            "package",
            "lints",
        ] {
            workspace.remove(key);
        }
        if let Some(metadata) = workspace
            .get_mut("metadata")
            .and_then(toml::Value::as_table_mut)
        {
            metadata.remove("affected");
            if metadata.is_empty() {
                workspace.remove("metadata");
            }
        }
    }
    toml::Value::Table(table)
}
fn normalize(value: Value, root: &Path) -> Value {
    match value {
        Value::String(s) => Value::String(s.replace(
            root.to_str().expect("UTF-8 snapshot directory"),
            "<repository>",
        )),
        Value::Array(values) => {
            Value::Array(values.into_iter().map(|v| normalize(v, root)).collect())
        }
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(k, v)| (k, normalize(v, root)))
                .collect(),
        ),
        value => value,
    }
}
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .with_context(|| format!("Cargo metadata missing string {key}"))
}
fn relative(root: &Path, path: &Path) -> Result<String> {
    // Cargo generally normalizes dependency paths, but also handle lexical ..
    // without requiring that a deleted input exists.
    let mut normalized = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            component => normalized.push(component),
        }
    }
    if let Ok(path) = normalized.strip_prefix(root) {
        return Ok(path
            .to_str()
            .context("package path is not UTF-8")?
            .replace('\\', "/"));
    }
    bail!(
        "local dependency {} is outside the repository snapshot; vendor it into the repository",
        path.display()
    )
}
