use crate::snapshot::Snapshot;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub type Selection = BTreeMap<String, BTreeSet<String>>;

pub fn analyze(base: &Snapshot, head: &Snapshot, changed: &[String]) -> Selection {
    let mut reasons: Selection = BTreeMap::new();
    let mut reverse: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for snapshot in [base, head] {
        for (id, package) in &snapshot.packages {
            for dependency in &package.dependencies {
                reverse
                    .entry(dependency.clone())
                    .or_default()
                    .insert(id.clone());
            }
        }
    }
    for id in base.packages.keys().chain(head.packages.keys()) {
        let reason = match (base.packages.get(id), head.packages.get(id)) {
            (Some(a), Some(b)) => {
                if a.manifest != b.manifest || a.metadata != b.metadata || a.lints != b.lints {
                    Some("package manifest or inherited settings changed")
                } else if a.dependencies != b.dependencies || a.features != b.features {
                    Some("resolved dependencies or features changed")
                } else {
                    None
                }
            }
            _ => Some("package added, removed, or dependency version changed"),
        };
        if let Some(reason) = reason {
            reasons.entry(id.clone()).or_default().insert(reason.into());
        }
    }
    if base.global != head.global {
        select_all(&mut reasons, head, "workspace build settings changed");
    }
    if base.rules_config != head.rules_config {
        select_all(&mut reasons, head, "shared-file selection rules changed");
    }
    for path in changed {
        if global_file(path) {
            select_all(
                &mut reasons,
                head,
                &format!("build configuration changed: {path}"),
            );
            continue;
        }
        let mut matched = false;
        for snapshot in [base, head] {
            for rule in &snapshot.rules {
                if !rule.paths.is_match(path) {
                    continue;
                }
                matched = true;
                for (id, package) in &snapshot.packages {
                    if package.member
                        && rule
                            .packages
                            .iter()
                            .any(|name| name == "*" || name == &package.name)
                    {
                        reasons
                            .entry(id.clone())
                            .or_default()
                            .insert(format!("shared input changed: {path}"));
                    }
                }
            }
        }
        // Manifests and the workspace lockfile were compared semantically above.
        if [base, head].iter().any(|s| {
            path == &s.manifest_path
                || path == &s.lock_path
                || s.packages
                    .values()
                    .any(|p| p.manifest_path.as_ref() == Some(path))
        }) {
            continue;
        }
        let mut owned = false;
        for snapshot in [base, head] {
            let owner = snapshot
                .packages
                .iter()
                .filter(|(_, p)| p.directory.as_ref().is_some_and(|d| owns(d, path)))
                .max_by_key(|(_, p)| p.directory.as_ref().map(String::len));
            if let Some((id, _)) = owner {
                owned = true;
                reasons
                    .entry(id.clone())
                    .or_default()
                    .insert(format!("changed file: {path}"));
            }
        }
        if !owned && !matched {
            select_all(
                &mut reasons,
                head,
                &format!("unmapped repository file changed: {path}"),
            );
        }
    }
    let mut queue: VecDeque<_> = reasons.keys().cloned().collect();
    let mut visited = BTreeSet::new();
    while let Some(id) = queue.pop_front() {
        if !visited.insert(id.clone()) {
            continue;
        }
        for dependent in reverse.get(&id).into_iter().flatten() {
            reasons
                .entry(dependent.clone())
                .or_default()
                .insert(format!("depends on {id}"));
            queue.push_back(dependent.clone());
        }
    }
    head.packages
        .iter()
        .filter(|(_, p)| p.member)
        .filter_map(|(id, p)| reasons.remove(id).map(|reasons| (p.name.clone(), reasons)))
        .collect()
}
fn select_all(reasons: &mut Selection, snapshot: &Snapshot, reason: &str) {
    for (id, package) in &snapshot.packages {
        if package.member {
            reasons.entry(id.clone()).or_default().insert(reason.into());
        }
    }
}
fn owns(directory: &str, path: &str) -> bool {
    directory.is_empty()
        || path
            .strip_prefix(directory)
            .is_some_and(|rest| rest.starts_with('/'))
}
fn global_file(path: &str) -> bool {
    let filename = path.rsplit('/').next().unwrap_or(path);
    matches!(filename, "rust-toolchain" | "rust-toolchain.toml")
        || path == ".cargo/config"
        || path == ".cargo/config.toml"
        || path.ends_with("/.cargo/config")
        || path.ends_with("/.cargo/config.toml")
}
