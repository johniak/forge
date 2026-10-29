//! Project (`forge.toml`) and module (`forge.module.toml`) manifests (ADR-0005).
//!
//! Loading never stops at the first problem: every diagnostic that can be
//! determined is returned together, each with a source span.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml::Spanned;

use crate::report::{Diagnostic, Span};

/// File name of the project manifest.
pub const PROJECT_MANIFEST: &str = "forge.toml";
/// File name of a module manifest.
pub const MODULE_MANIFEST: &str = "forge.module.toml";
/// The only project manifest version this crate understands.
pub const SUPPORTED_MANIFEST_VERSION: u32 = 1;

/// What a module is, which later selects layering rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleKind {
    /// Business domain app (models, actions, policies, …).
    Domain,
    /// Developer tooling core.
    Tooling,
    /// Command-line adapter.
    Cli,
    /// Framework runtime.
    Runtime,
    /// Installable application package.
    App,
    /// Cross-cutting shared contracts.
    Shared,
}

/// Verification obligations declared by a module (SPEC §7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verification {
    /// Required line coverage, percent.
    #[serde(default = "full")]
    pub line_coverage: u8,
    /// Required branch coverage, percent.
    #[serde(default = "full")]
    pub branch_coverage: u8,
}

fn full() -> u8 {
    100
}

impl Default for Verification {
    fn default() -> Self {
        Self {
            line_coverage: full(),
            branch_coverage: full(),
        }
    }
}

/// A loaded, validated module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// Name as declared in `forge.toml` (and matching `forge.module.toml`).
    pub name: String,
    /// Canonical absolute directory.
    pub path: PathBuf,
    /// Kind of module.
    pub kind: ModuleKind,
    /// Free-text description.
    pub description: String,
    /// Modules this one may depend on.
    pub allowed: Vec<String>,
    /// Modules this one must not depend on.
    pub forbidden: Vec<String>,
    /// Coverage obligations.
    pub verification: Verification,
}

/// A loaded, validated project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// Canonical absolute project root (the directory holding `forge.toml`).
    pub root: PathBuf,
    /// Project name.
    pub name: String,
    /// Modules in `forge.toml` order (alphabetical by name).
    pub modules: Vec<Module>,
}

#[derive(Deserialize)]
struct ProjectFile {
    project: ProjectSection,
    #[serde(default)]
    modules: BTreeMap<String, Spanned<String>>,
}

#[derive(Deserialize)]
struct ProjectSection {
    name: String,
    manifest_version: Spanned<u32>,
}

#[derive(Deserialize)]
struct ModuleFile {
    module: ModuleSection,
    #[serde(default)]
    dependencies: DependenciesSection,
    #[serde(default)]
    verification: Verification,
}

#[derive(Deserialize)]
struct ModuleSection {
    name: Spanned<String>,
    kind: ModuleKind,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize, Default)]
struct DependenciesSection {
    #[serde(default)]
    allowed: Vec<Spanned<String>>,
    #[serde(default)]
    forbidden: Vec<Spanned<String>>,
}

/// Parses TOML text into `T`, mapping syntax and shape errors to one
/// diagnostic carrying the error's span.
pub(crate) fn parse_toml<T: serde::de::DeserializeOwned>(
    file: &Path,
    text: &str,
    code: &str,
) -> Result<T, Box<Diagnostic>> {
    toml::from_str(text).map_err(|err: toml::de::Error| {
        let diag = Diagnostic::error(code, format!("{}: {}", file.display(), err.message()));
        Box::new(match err.span() {
            Some(range) => diag.with_span(Span::from_byte_range(file.to_path_buf(), text, range)),
            None => diag,
        })
    })
}

fn read_error(code: &str, file: &Path, err: &std::io::Error) -> Diagnostic {
    Diagnostic::error(code, format!("cannot read {}: {err}", file.display()))
}

/// Loads and validates the project rooted at `root`.
///
/// # Errors
///
/// Returns every diagnostic found (`FM001`…`FM007`); the list is never empty.
pub fn load_project(root: &Path) -> Result<Project, Vec<Diagnostic>> {
    let root = root
        .canonicalize()
        .map_err(|err| vec![read_error("FM001", &root.join(PROJECT_MANIFEST), &err)])?;
    let manifest_path = root.join(PROJECT_MANIFEST);
    let text = fs::read_to_string(&manifest_path)
        .map_err(|err| vec![read_error("FM001", &manifest_path, &err)])?;
    let file: ProjectFile = parse_toml(&manifest_path, &text, "FM001").map_err(|d| vec![*d])?;

    let mut diags = Vec::new();
    if *file.project.manifest_version.get_ref() != SUPPORTED_MANIFEST_VERSION {
        diags.push(
            Diagnostic::error(
                "FM007",
                format!(
                    "unsupported manifest_version {} (this tool understands {SUPPORTED_MANIFEST_VERSION})",
                    file.project.manifest_version.get_ref()
                ),
            )
            .with_span(Span::from_byte_range(manifest_path.clone(), &text, file.project.manifest_version.span())),
        );
    }

    let declared: BTreeSet<&str> = file.modules.keys().map(String::as_str).collect();
    let mut seen_paths: BTreeMap<PathBuf, &str> = BTreeMap::new();
    let mut modules = Vec::new();

    for (name, rel_path) in &file.modules {
        let entry_span = Span::from_byte_range(manifest_path.clone(), &text, rel_path.span());
        let dir = root.join(rel_path.get_ref());
        let key = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if let Some(other) = seen_paths.insert(key.clone(), name) {
            diags.push(
                Diagnostic::error(
                    "FM004",
                    format!(
                        "modules `{other}` and `{name}` share the path {}",
                        rel_path.get_ref()
                    ),
                )
                .with_span(entry_span),
            );
            continue;
        }
        match load_module(name, key, &declared, entry_span) {
            Ok(module) => modules.push(module),
            Err(module_diags) => diags.extend(module_diags),
        }
    }

    if diags.is_empty() {
        Ok(Project {
            root,
            name: file.project.name,
            modules,
        })
    } else {
        Err(diags)
    }
}

/// Reads and validates one module manifest in directory `dir` (canonical when
/// it exists). `entry_span` points at the module's entry in `forge.toml`.
fn load_module(
    name: &str,
    dir: PathBuf,
    declared: &BTreeSet<&str>,
    entry_span: Span,
) -> Result<Module, Vec<Diagnostic>> {
    let module_manifest = dir.join(MODULE_MANIFEST);
    let module_text = fs::read_to_string(&module_manifest).map_err(|err| {
        vec![
            Diagnostic::error(
                "FM002",
                format!(
                    "module `{name}`: cannot read {}: {err}",
                    module_manifest.display()
                ),
            )
            .with_span(entry_span),
        ]
    })?;
    let parsed: ModuleFile =
        parse_toml(&module_manifest, &module_text, "FM001").map_err(|d| vec![*d])?;
    if parsed.module.name.get_ref() != name {
        return Err(vec![
            Diagnostic::error(
                "FM003",
                format!(
                    "module manifest declares name `{}` but forge.toml calls it `{name}`",
                    parsed.module.name.get_ref()
                ),
            )
            .with_span(Span::from_byte_range(
                module_manifest.clone(),
                &module_text,
                parsed.module.name.span(),
            )),
        ]);
    }
    let span_of = |entry: &Spanned<String>| {
        Span::from_byte_range(module_manifest.clone(), &module_text, entry.span())
    };
    let (allowed, forbidden) =
        validate_dependencies(name, &parsed.dependencies, declared, span_of)?;
    Ok(Module {
        name: name.to_string(),
        path: dir,
        kind: parsed.module.kind,
        description: parsed.module.description,
        allowed,
        forbidden,
        verification: parsed.verification,
    })
}

/// Checks a module's `allowed`/`forbidden` lists against the declared modules
/// (`FM005`, `FM006`) and returns them as plain names.
fn validate_dependencies(
    name: &str,
    deps: &DependenciesSection,
    declared: &BTreeSet<&str>,
    span_of: impl Fn(&Spanned<String>) -> Span,
) -> Result<(Vec<String>, Vec<String>), Vec<Diagnostic>> {
    let mut diags = Vec::new();
    let unknown = |entry: &Spanned<String>, list: &str| {
        Diagnostic::error(
            "FM005",
            format!(
                "module `{name}`: `{}` in `{list}` is not a module of this project",
                entry.get_ref()
            ),
        )
        .with_span(span_of(entry))
    };
    let mut allowed = Vec::new();
    for entry in &deps.allowed {
        let target = entry.get_ref();
        if target == name || !declared.contains(target.as_str()) {
            diags.push(unknown(entry, "allowed"));
        }
        allowed.push(target.clone());
    }
    let mut forbidden = Vec::new();
    for entry in &deps.forbidden {
        let target = entry.get_ref();
        if allowed.contains(target) {
            diags.push(
                Diagnostic::error(
                    "FM006",
                    format!("module `{name}`: `{target}` is both allowed and forbidden"),
                )
                .with_span(span_of(entry)),
            );
        } else if target == name || !declared.contains(target.as_str()) {
            diags.push(unknown(entry, "forbidden"));
        }
        forbidden.push(target.clone());
    }
    if diags.is_empty() {
        Ok((allowed, forbidden))
    } else {
        Err(diags)
    }
}
