use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tempfile::TempDir;

struct Repo(TempDir);
impl Repo {
    fn new() -> Self {
        let repo = Self(tempfile::tempdir().unwrap());
        repo.git(&["init", "-b", "main"]);
        repo.git(&["config", "user.email", "test@example.com"]);
        repo.git(&["config", "user.name", "Test"]);
        repo.write(
            "Cargo.toml",
            "[workspace]\nresolver = '2'\nmembers = ['crates/*']\n",
        );
        repo
    }
    fn path(&self) -> &Path {
        self.0.path()
    }
    fn write(&self, path: &str, content: &str) {
        let path = self.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    }
    fn package(&self, name: &str, dependencies: &str) {
        self.write(
            &format!("crates/{name}/Cargo.toml"),
            &format!(
                "[package]\nname = '{name}'\nversion = '0.1.0'\nedition = '2021'\n{dependencies}"
            ),
        );
        self.write(
            &format!("crates/{name}/src/lib.rs"),
            "pub fn value() -> u32 { 1 }\n",
        );
    }
    fn commit(&self) -> String {
        let out = Command::new("cargo")
            .args(["generate-lockfile", "--offline"])
            .current_dir(self.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        self.git(&["add", "."]);
        self.git(&[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--no-gpg-sign",
            "-m",
            "fixture",
        ]);
        self.git(&["rev-parse", "HEAD"])
    }
    fn run(&self, base: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cargo-affected"))
            .args(["affected", "--base", base, "--format", "json", "--offline"])
            .args(args)
            .current_dir(self.path())
            .output()
            .unwrap()
    }
    fn affected(&self, base: &str) -> Vec<String> {
        let out = self.run(base, &[]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)))
    }
}

fn chain() -> Repo {
    let r = Repo::new();
    r.package("core", "");
    r.package("middle", "[dependencies]\ncore = { path = '../core' }\n");
    r.package("app", "[dependencies]\nmiddle = { path = '../middle' }\n");
    r.package("other", "");
    r
}

#[test]
fn changed_leaf_selects_transitive_dependents_only() {
    let r = chain();
    let base = r.commit();
    r.write("crates/core/src/lib.rs", "pub fn value() -> u32 { 2 }\n");
    r.commit();
    assert_eq!(r.affected(&base), ["app", "core", "middle"]);
}

#[test]
fn unchanged_revision_is_empty() {
    let r = chain();
    let base = r.commit();
    assert!(r.affected(&base).is_empty());
}

#[test]
fn root_dependency_change_selects_only_consumers() {
    let r = Repo::new();
    r.write("Cargo.toml", "[workspace]\nresolver='2'\nmembers=['crates/*']\n[workspace.dependencies]\ncore={path='crates/core', features=[]}\n");
    r.package("core", "[features]\nextra=[]\n");
    r.package("app", "[dependencies]\ncore.workspace=true\n");
    r.package("other", "");
    let base = r.commit();
    r.write("Cargo.toml", "[workspace]\nresolver='2'\nmembers=['crates/*']\n[workspace.dependencies]\ncore={path='crates/core', features=['extra']}\n");
    r.commit();
    assert_eq!(r.affected(&base), ["app"]);
}

#[test]
fn inherited_package_fields_and_lints_are_scoped() {
    for (table, first, second, inherit) in [
        (
            "package",
            "edition='2018'",
            "edition='2021'",
            "edition.workspace=true",
        ),
        (
            "lints.rust",
            "unsafe_code='warn'",
            "unsafe_code='deny'",
            "edition='2021'\n[lints]\nworkspace=true",
        ),
    ] {
        let r = Repo::new();
        let root = |settings| {
            format!(
                "[workspace]\nresolver='2'\nmembers=['crates/*']\n[workspace.{table}]\n{settings}\n"
            )
        };
        r.write("Cargo.toml", &root(first));
        r.package("other", "");
        r.write(
            "crates/inherits/Cargo.toml",
            &format!("[package]\nname='inherits'\nversion='0.1.0'\n{inherit}\n"),
        );
        r.write("crates/inherits/src/lib.rs", "");
        let base = r.commit();
        r.write("Cargo.toml", &root(second));
        r.commit();
        assert_eq!(r.affected(&base), ["inherits"], "{table}");
    }
}

#[test]
fn formatting_and_unused_workspace_dependencies_do_not_select_packages() {
    let r = chain();
    let base = r.commit();
    r.write("Cargo.toml", "# comment\n[workspace]\nresolver='2'\nmembers=['crates/*']\n[workspace.dependencies]\nunused='1'\n");
    r.write(
        "crates/core/Cargo.toml",
        "# comment\n[package]\nedition='2021'\nversion='0.1.0'\nname='core'\n",
    );
    r.commit();
    assert_eq!(r.affected(&base), Vec::<String>::new());
}

#[test]
fn adding_a_member_does_not_select_existing_members() {
    let r = chain();
    let base = r.commit();
    r.package("new", "");
    r.commit();
    assert_eq!(r.affected(&base), ["new"]);
}

#[test]
fn deleted_dependency_propagates_over_old_edges() {
    let r = chain();
    let base = r.commit();
    fs::remove_dir_all(r.path().join("crates/core")).unwrap();
    r.package("middle", "");
    r.commit();
    assert_eq!(r.affected(&base), ["app", "middle"]);
}

#[test]
fn all_dependency_kinds_and_target_conditions_are_followed() {
    let r = Repo::new();
    r.package("leaf", "");
    r.package(
        "build-user",
        "[build-dependencies]\nleaf={path='../leaf'}\n",
    );
    r.package("dev-user", "[dev-dependencies]\nleaf={path='../leaf'}\n");
    r.package(
        "optional-user",
        "[dependencies]\nalias={package='leaf',path='../leaf',optional=true}\n",
    );
    r.package(
        "target-user",
        "[target.'cfg(windows)'.dependencies]\nleaf={path='../leaf'}\n",
    );
    r.package("other", "");
    let base = r.commit();
    r.write("crates/leaf/src/lib.rs", "// changed\n");
    r.commit();
    assert_eq!(
        r.affected(&base),
        [
            "build-user",
            "dev-user",
            "leaf",
            "optional-user",
            "target-user"
        ]
    );
}

#[test]
fn global_settings_select_every_member() {
    for (path, content) in [
        (
            "Cargo.toml",
            "[workspace]\nresolver='2'\nmembers=['crates/*']\n[profile.test]\nopt-level=1\n",
        ),
        (
            ".cargo/config.toml",
            "[build]\nrustflags=['--cfg', 'something']\n",
        ),
        ("rust-toolchain.toml", "[toolchain]\nchannel='stable'\n"),
    ] {
        let r = chain();
        let base = r.commit();
        r.write(path, content);
        r.commit();
        assert_eq!(
            r.affected(&base),
            ["app", "core", "middle", "other"],
            "{path}"
        );
    }
}

#[test]
fn unowned_files_default_to_all_members() {
    let r = chain();
    let base = r.commit();
    r.write("schemas/input.json", "{}");
    r.commit();
    assert_eq!(r.affected(&base), ["app", "core", "middle", "other"]);
}

#[test]
fn shared_rules_scope_inputs_and_allow_explicit_ignores() {
    let r = chain();
    r.write("Cargo.toml", "[workspace]\nresolver='2'\nmembers=['crates/*']\n[[workspace.metadata.affected.rules]]\npaths=['schemas/**']\npackages=['middle']\n[[workspace.metadata.affected.rules]]\npaths=['docs/**']\npackages=[]\n");
    let base = r.commit();
    r.write("schemas/input.json", "{}");
    r.write("docs/guide.md", "docs");
    r.commit();
    assert_eq!(r.affected(&base), ["app", "middle"]);
}

#[test]
fn invalid_rule_is_an_error_not_an_empty_selection() {
    let r = chain();
    r.write("Cargo.toml", "[workspace]\nresolver='2'\nmembers=['crates/*']\n[[workspace.metadata.affected.rules]]\npaths=['shared/**']\npackages=['typo']\n");
    let base = r.commit();
    let out = r.run(&base, &[]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
}

#[test]
fn merge_base_ignores_changes_only_on_the_base_branch() {
    let r = chain();
    r.commit();
    r.git(&["checkout", "-b", "feature"]);
    r.write("crates/core/src/lib.rs", "// feature\n");
    r.commit();
    r.git(&["checkout", "main"]);
    r.write("crates/other/src/lib.rs", "// main\n");
    r.commit();
    r.git(&["checkout", "feature"]);
    assert_eq!(r.affected("main"), ["app", "core", "middle"]);
    let out = r.run("main", &["--exact"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Vec<String>>(&out.stdout).unwrap(),
        ["app", "core", "middle", "other"]
    );
}

#[test]
fn invalid_revision_fails_and_explanations_do_not_pollute_json() {
    let r = chain();
    let base = r.commit();
    let bad = r.run("not-a-revision", &[]);
    assert!(!bad.status.success());
    assert!(bad.stdout.is_empty());
    r.write("crates/core/src/lib.rs", "// changed\n");
    r.commit();
    let out = r.run(&base, &["--explain"]);
    assert!(out.status.success());
    assert_eq!(
        serde_json::from_slice::<Vec<String>>(&out.stdout).unwrap(),
        ["app", "core", "middle"]
    );
    let explain = String::from_utf8_lossy(&out.stderr);
    assert!(explain.contains("crates/core/src/lib.rs"));
    assert!(explain.contains("app:"));
}

#[test]
fn root_package_does_not_own_nested_members_or_shared_workspace_settings() {
    let r = Repo::new();
    r.write("Cargo.toml", "[package]\nname='root'\nversion='0.1.0'\nedition='2021'\n[workspace]\nresolver='2'\nmembers=['crates/*']\n[workspace.package]\nedition='2018'\n");
    r.write("src/lib.rs", "");
    r.write(
        "crates/child/Cargo.toml",
        "[package]\nname='child'\nversion='0.1.0'\nedition.workspace=true\n",
    );
    r.write("crates/child/src/lib.rs", "");
    let base = r.commit();
    let manifest = fs::read_to_string(r.path().join("Cargo.toml"))
        .unwrap()
        .replace("edition='2018'", "edition='2021'");
    r.write("Cargo.toml", &manifest);
    r.commit();
    assert_eq!(r.affected(&base), ["child"]);
    let base = r.git(&["rev-parse", "HEAD"]);
    r.write("crates/child/src/lib.rs", "// changed\n");
    r.commit();
    assert_eq!(r.affected(&base), ["child"]);
}

#[test]
fn excluded_local_dependency_changes_select_its_workspace_consumers() {
    let r = Repo::new();
    r.write(
        "Cargo.toml",
        "[workspace]\nresolver='2'\nmembers=['crates/*']\nexclude=['vendor/leaf']\n",
    );
    r.write(
        "vendor/leaf/Cargo.toml",
        "[package]\nname='leaf'\nversion='0.1.0'\nedition='2021'\n",
    );
    r.write("vendor/leaf/src/lib.rs", "");
    r.package("app", "[dependencies]\nleaf={path='../../vendor/leaf'}\n");
    r.package("other", "");
    let base = r.commit();
    r.write("vendor/leaf/src/lib.rs", "// changed\n");
    r.commit();
    assert_eq!(r.affected(&base), ["app"]);
}

#[test]
fn renamed_crate_selects_current_name_and_old_dependents() {
    let r = chain();
    let base = r.commit();
    r.git(&["mv", "crates/core", "crates/renamed"]);
    r.package("renamed", "");
    r.package(
        "middle",
        "[dependencies]\ncore={package='renamed',path='../renamed'}\n",
    );
    r.commit();
    assert_eq!(r.affected(&base), ["app", "middle", "renamed"]);
}

#[test]
fn new_workspace_selects_all_current_members() {
    let r = Repo::new();
    fs::remove_file(r.path().join("Cargo.toml")).unwrap();
    r.write("README.md", "initial\n");
    r.git(&["add", "."]);
    r.git(&["-c", "commit.gpgsign=false", "commit", "-m", "initial"]);
    let base = r.git(&["rev-parse", "HEAD"]);
    r.write(
        "Cargo.toml",
        "[workspace]\nresolver='2'\nmembers=['crates/*']\n",
    );
    r.package("first", "");
    r.package("second", "");
    r.commit();
    assert_eq!(r.affected(&base), ["first", "second"]);
}

#[test]
fn uncommitted_changes_are_excluded_and_dirty_checkout_is_untouched() {
    let r = chain();
    let base = r.commit();
    r.write("crates/core/src/lib.rs", "// dirty\n");
    r.write("crates/new/Cargo.toml", "not valid TOML");
    let before = r.git(&["status", "--porcelain"]);
    assert!(r.affected(&base).is_empty());
    assert_eq!(r.git(&["status", "--porcelain"]), before);
}

#[test]
fn invalid_committed_manifest_is_an_error() {
    let r = chain();
    let base = r.commit();
    r.write("crates/core/Cargo.toml", "not valid TOML");
    r.git(&["add", "."]);
    r.git(&["-c", "commit.gpgsign=false", "commit", "-m", "broken"]);
    let out = r.run(&base, &[]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("head revision"));
}

#[test]
fn nested_workspace_manifest_paths_are_supported() {
    let r = Repo::new();
    fs::remove_file(r.path().join("Cargo.toml")).unwrap();
    r.write(
        "rust/Cargo.toml",
        "[workspace]\nresolver='2'\nmembers=['app','other']\n",
    );
    for name in ["app", "other"] {
        r.write(
            &format!("rust/{name}/Cargo.toml"),
            &format!("[package]\nname='{name}'\nversion='0.1.0'\nedition='2021'\n"),
        );
        r.write(&format!("rust/{name}/src/lib.rs"), "");
    }
    let lock = Command::new("cargo")
        .current_dir(r.path())
        .args([
            "generate-lockfile",
            "--offline",
            "--manifest-path",
            "rust/Cargo.toml",
        ])
        .output()
        .unwrap();
    assert!(lock.status.success());
    r.git(&["add", "."]);
    r.git(&["-c", "commit.gpgsign=false", "commit", "-m", "base"]);
    let base = r.git(&["rev-parse", "HEAD"]);
    r.write("rust/app/src/lib.rs", "// changed\n");
    r.git(&["add", "."]);
    r.git(&["-c", "commit.gpgsign=false", "commit", "-m", "head"]);
    let out = r.run(&base, &["--manifest-path", "rust/Cargo.toml"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Vec<String>>(&out.stdout).unwrap(),
        ["app"]
    );
}

#[test]
fn dev_dependency_cycles_terminate() {
    let r = Repo::new();
    r.package("a", "[dev-dependencies]\nb={path='../b'}\n");
    r.package("b", "[dependencies]\na={path='../a'}\n");
    r.package("other", "");
    let base = r.commit();
    r.write("crates/a/src/lib.rs", "// changed\n");
    r.commit();
    assert_eq!(r.affected(&base), ["a", "b"]);
}

#[test]
fn lockfile_git_revision_change_selects_only_consumers() {
    let upstream = Repo::new();
    upstream.package("external", "");
    upstream.commit();
    let r = chain();
    r.package(
        "core",
        &format!(
            "[dependencies]\nexternal={{git='file://{}'}}\n",
            upstream.path().display()
        ),
    );
    let cargo_home = tempfile::tempdir().unwrap();
    let update = || {
        let out = Command::new("cargo")
            .args(["update", "-p", "external"])
            .env("CARGO_HOME", cargo_home.path())
            .current_dir(r.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    update();
    r.git(&["add", "."]);
    r.git(&["-c", "commit.gpgsign=false", "commit", "-m", "base"]);
    let base = r.git(&["rev-parse", "HEAD"]);
    upstream.write("crates/external/src/lib.rs", "// new implementation\n");
    upstream.commit();
    update();
    r.git(&["add", "."]);
    r.git(&[
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-m",
        "lockfile update",
    ]);
    assert_eq!(r.git(&["diff", "--name-only", &base, "HEAD"]), "Cargo.lock");
    let out = Command::new(env!("CARGO_BIN_EXE_cargo-affected"))
        .args(["--base", &base, "--format", "json", "--offline"])
        .env("CARGO_HOME", cargo_home.path())
        .current_dir(r.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Vec<String>>(&out.stdout).unwrap(),
        ["app", "core", "middle"]
    );
}

#[test]
fn export_attributes_cannot_hide_changed_workspace_members() {
    let r = chain();
    r.write(".gitattributes", "crates/core export-ignore\n");
    let base = r.commit();
    r.write("crates/core/src/lib.rs", "// changed\n");
    r.commit();
    assert_eq!(r.affected(&base), ["app", "core", "middle"]);
}

#[test]
fn shared_rules_also_apply_to_manifests_and_lockfiles() {
    let r = chain();
    r.write("Cargo.toml", "[workspace]\nresolver='2'\nmembers=['crates/*']\n[[workspace.metadata.affected.rules]]\npaths=['crates/core/Cargo.toml','Cargo.lock']\npackages=['other']\n");
    let base = r.commit();
    r.package("core", "[package.metadata]\nmeaning=42\n");
    r.commit();
    assert_eq!(r.affected(&base), ["app", "core", "middle", "other"]);
    let base = r.git(&["rev-parse", "HEAD"]);
    let lock = fs::read_to_string(r.path().join("Cargo.lock")).unwrap();
    r.write("Cargo.lock", &format!("# changed comment\n{lock}"));
    r.git(&["add", "."]);
    r.git(&["-c", "commit.gpgsign=false", "commit", "-m", "lock comment"]);
    assert_eq!(r.affected(&base), ["other"]);
}

#[test]
fn excluded_dependency_with_own_workspace_lints_is_tracked() {
    let r = Repo::new();
    r.write(
        "Cargo.toml",
        "[workspace]\nresolver='2'\nmembers=['crates/*']\nexclude=['vendor/leaf']\n",
    );
    let leaf = |level| {
        format!(
            "[package]\nname='leaf'\nversion='0.1.0'\nedition='2021'\n[lints]\nworkspace=true\n[workspace]\n[workspace.lints.rust]\nunsafe_code='{level}'\n"
        )
    };
    r.write("vendor/leaf/Cargo.toml", &leaf("warn"));
    r.write("vendor/leaf/src/lib.rs", "");
    r.package("app", "[dependencies]\nleaf={path='../../vendor/leaf'}\n");
    r.package("other", "");
    let base = r.commit();
    r.write("vendor/leaf/Cargo.toml", &leaf("deny"));
    r.commit();
    assert_eq!(r.affected(&base), ["app"]);
}

#[test]
fn workspace_metadata_is_a_shared_build_input() {
    let r = chain();
    let base = r.commit();
    r.write("Cargo.toml", "[workspace]\nresolver='2'\nmembers=['crates/*']\n[workspace.metadata.build]\nmessage='new'\n");
    r.commit();
    assert_eq!(r.affected(&base), ["app", "core", "middle", "other"]);
}

#[test]
fn sha256_git_repositories_are_supported() {
    let r = Repo(tempfile::tempdir().unwrap());
    r.git(&["init", "-b", "main", "--object-format=sha256"]);
    r.git(&["config", "user.email", "test@example.com"]);
    r.git(&["config", "user.name", "Test"]);
    r.write(
        "Cargo.toml",
        "[workspace]\nresolver='2'\nmembers=['crates/*']\n",
    );
    r.package("app", "");
    let base = r.commit();
    r.write("crates/app/src/lib.rs", "// changed\n");
    r.commit();
    assert_eq!(r.affected(&base), ["app"]);
}

#[test]
fn temporary_directory_spaces_do_not_create_spurious_metadata_changes() {
    let r = chain();
    let base = r.commit();
    let temporary = tempfile::Builder::new()
        .prefix("cargo affected ")
        .tempdir()
        .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cargo-affected"))
        .args(["--base", &base, "--format", "json", "--offline"])
        .env("TMPDIR", temporary.path())
        .current_dir(r.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Vec<String>>(&out.stdout).unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn lints_from_a_separate_virtual_workspace_select_consumers() {
    let r = Repo::new();
    r.write("Cargo.toml", "[package]\nname='root'\nversion='0.1.0'\nedition='2021'\n[workspace]\nresolver='2'\nmembers=['crates/*']\nexclude=['vendor']\n");
    r.write("src/lib.rs", "");
    let vendor = |level| {
        format!(
            "[workspace]\nresolver='2'\nmembers=['leaf']\n[workspace.lints.rust]\nunsafe_code='{level}'\n"
        )
    };
    r.write("vendor/Cargo.toml", &vendor("warn"));
    r.write(
        "vendor/leaf/Cargo.toml",
        "[package]\nname='leaf'\nversion='0.1.0'\nedition='2021'\n[lints]\nworkspace=true\n",
    );
    r.write("vendor/leaf/src/lib.rs", "");
    r.package("app", "[dependencies]\nleaf={path='../../vendor/leaf'}\n");
    r.package("other", "");
    let base = r.commit();
    r.write("vendor/Cargo.toml", &vendor("deny"));
    r.commit();
    assert_eq!(r.affected(&base), ["app", "root"]);
}

#[test]
fn export_substitution_does_not_invent_manifest_changes() {
    let r = chain();
    r.package("other", "[package.metadata]\ncommit='$Format:%H$'\n");
    r.write(".gitattributes", "crates/other/Cargo.toml export-subst\n");
    let base = r.commit();
    r.write("crates/core/src/lib.rs", "// changed\n");
    r.commit();
    assert_eq!(r.affected(&base), ["app", "core", "middle"]);
}
