use anyhow::{Context, Result, bail};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;

pub struct Repository {
    pub root: PathBuf,
}

pub fn output(command: &mut Command) -> Result<Vec<u8>> {
    let result = command
        .output()
        .with_context(|| format!("could not run {command:?}"))?;
    if !result.status.success() {
        bail!(
            "{command:?} failed: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        );
    }
    Ok(result.stdout)
}

impl Repository {
    pub fn open(path: &Path) -> Result<Self> {
        let root = output(
            Command::new("git")
                .current_dir(path)
                .args(["rev-parse", "--show-toplevel"]),
        )?;
        Ok(Self {
            root: PathBuf::from(String::from_utf8(root)?.trim_end()),
        })
    }
    fn git(&self) -> Command {
        let mut command = Command::new("git");
        command.current_dir(&self.root);
        command
    }
    pub fn resolve(&self, reference: &str) -> Result<String> {
        let bytes = output(self.git().args(["rev-parse", "--verify", "--end-of-options", &format!("{reference}^{{commit}}")]))
            .with_context(|| format!("cannot resolve {reference:?}; fetch the base and head commits (Actions checkout needs fetch-depth: 0)"))?;
        Ok(String::from_utf8(bytes)?.trim().into())
    }
    pub fn merge_base(&self, base: &str, head: &str) -> Result<String> {
        Ok(String::from_utf8(
            output(self.git().args(["merge-base", base, head]))
                .context("no merge base; fetch sufficient Git history or use --exact")?,
        )?
        .trim()
        .into())
    }
    pub fn export(&self, revision: &str) -> Result<TempDir> {
        // Archive through a disposable object reader so repository export-ignore
        // and export-subst attributes cannot omit or rewrite committed inputs.
        // This never writes to the user's Git configuration or object database.
        let reader = tempfile::tempdir()?;
        let object_format = String::from_utf8(output(
            self.git().args(["rev-parse", "--show-object-format"]),
        )?)?;
        output(
            Command::new("git")
                .args(["init", "--bare", "--quiet"])
                .arg(format!("--object-format={}", object_format.trim()))
                .arg(reader.path()),
        )?;
        let objects = output(self.git().args([
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "objects",
        ]))?;
        std::fs::write(reader.path().join("objects/info/alternates"), objects)?;
        std::fs::write(
            reader.path().join("info/attributes"),
            "* -export-ignore -export-subst\n",
        )?;
        let archive = output(
            Command::new("git")
                .arg("--git-dir")
                .arg(reader.path())
                .args(["archive", "--format=tar", revision]),
        )?;
        let directory = tempfile::tempdir()?;
        tar::Archive::new(archive.as_slice())
            .unpack(directory.path())
            .context("extracting Git snapshot")?;
        Ok(directory)
    }
    pub fn changed(&self, base: &str, head: &str) -> Result<Vec<String>> {
        let bytes = output(self.git().args([
            "diff",
            "--name-only",
            "--no-renames",
            "-z",
            base,
            head,
            "--",
        ]))?;
        bytes
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| String::from_utf8(p.to_vec()).context("changed file path is not UTF-8"))
            .collect()
    }
}
