//! Helpers shared by the integration tests: build throw-away Forge projects.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::missing_panics_doc,
    clippy::must_use_candidate
)]

use std::fs;
use std::path::{Path, PathBuf};

/// A temporary project directory that is deleted on drop.
pub struct TempProject {
    dir: tempfile::TempDir,
}

impl TempProject {
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Writes `content` to `rel` (relative to the root), creating parents.
    pub fn write(&self, rel: &str, content: &str) -> PathBuf {
        let path = self.root().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        path
    }

    /// Writes a module directory with a manifest, a Cargo.toml and an empty lib.
    pub fn module(
        &self,
        name: &str,
        path: &str,
        allowed: &[&str],
        forbidden: &[&str],
        cargo_deps: &str,
    ) {
        let quoted = |xs: &[&str]| {
            xs.iter()
                .map(|x| format!("\"{x}\""))
                .collect::<Vec<_>>()
                .join(", ")
        };
        self.write(
            &format!("{path}/forge.module.toml"),
            &format!(
                "[module]\nname = \"{name}\"\nkind = \"domain\"\ndescription = \"test module\"\n\n[dependencies]\nallowed = [{}]\nforbidden = [{}]\n\n[verification]\nline_coverage = 100\nbranch_coverage = 100\n",
                quoted(allowed),
                quoted(forbidden)
            ),
        );
        self.write(
            &format!("{path}/Cargo.toml"),
            &format!("[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\n{cargo_deps}\n"),
        );
        self.write(&format!("{path}/src/lib.rs"), "");
    }

    /// Writes `forge.toml` listing the given `(name, path)` modules.
    pub fn forge_toml(&self, modules: &[(&str, &str)]) {
        let mut s = String::from("[project]\nname = \"demo\"\nmanifest_version = 1\n\n[modules]\n");
        for (name, path) in modules {
            s.push_str(&format!("{name} = \"{path}\"\n"));
        }
        self.write("forge.toml", &s);
    }
}

/// Two modules: `core` (depends on nothing) and `cli` (allowed to depend on `core`,
/// and really does through a path dependency).
pub fn valid_project() -> TempProject {
    let tp = TempProject::new();
    tp.write(
        "Cargo.toml",
        "[workspace]\nresolver = \"3\"\nmembers = [\"crates/core\", \"crates/cli\"]\n",
    );
    tp.forge_toml(&[("core", "crates/core"), ("cli", "crates/cli")]);
    tp.module("core", "crates/core", &[], &["cli"], "");
    tp.module(
        "cli",
        "crates/cli",
        &["core"],
        &[],
        "core = { path = \"../core\" }",
    );
    tp
}
