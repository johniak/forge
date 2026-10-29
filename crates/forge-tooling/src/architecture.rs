//! Architecture guard: compares the **real** Cargo dependency graph of every
//! module with what its manifest allows (ADR-0005, SPEC §7.3).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use toml::{Spanned, Value};

use crate::manifest::{Module, Project, parse_toml};
use crate::report::{Diagnostic, Span};

#[derive(Deserialize, Default)]
struct CargoFile {
    #[serde(default)]
    dependencies: BTreeMap<String, Spanned<Value>>,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: BTreeMap<String, Spanned<Value>>,
    #[serde(default, rename = "build-dependencies")]
    build_dependencies: BTreeMap<String, Spanned<Value>>,
}

#[derive(Deserialize, Default)]
struct RootCargoFile {
    #[serde(default)]
    workspace: WorkspaceSection,
}

#[derive(Deserialize, Default)]
struct WorkspaceSection {
    #[serde(default)]
    dependencies: BTreeMap<String, Value>,
}

/// One real dependency edge between two modules.
struct Edge {
    from: String,
    to: String,
    section: &'static str,
    span: Span,
}

/// `[workspace.dependencies]` entries that point at a path, keyed by name.
fn workspace_dependencies(root: &Path) -> BTreeMap<String, PathBuf> {
    let file = root.join("Cargo.toml");
    let Ok(text) = fs::read_to_string(&file) else {
        return BTreeMap::new();
    };
    let Ok(parsed) = parse_toml::<RootCargoFile>(&file, &text, "FA004") else {
        return BTreeMap::new();
    };
    parsed
        .workspace
        .dependencies
        .into_iter()
        .filter_map(|(name, value)| path_of(&value).map(|p| (name, root.join(p))))
        .collect()
}

/// `path = "…"` of a dependency table, if any.
fn path_of(value: &Value) -> Option<&str> {
    value.as_table()?.get("path")?.as_str()
}

/// `workspace = true` of a dependency table.
fn is_workspace_dep(value: &Value) -> bool {
    value
        .as_table()
        .and_then(|t| t.get("workspace"))
        .and_then(Value::as_bool)
        == Some(true)
}

/// Real module-to-module edges declared in a module's `Cargo.toml`.
fn module_edges(
    module: &Module,
    workspace_deps: &BTreeMap<String, PathBuf>,
    by_path: &HashMap<&Path, &str>,
) -> Result<Vec<Edge>, Diagnostic> {
    let file = module.path.join("Cargo.toml");
    let text = fs::read_to_string(&file).map_err(|err| {
        Diagnostic::error(
            "FA004",
            format!(
                "module `{}`: cannot read {}: {err}",
                module.name,
                file.display()
            ),
        )
    })?;
    let parsed: CargoFile = parse_toml(&file, &text, "FA004")?;

    let sections = [
        ("dependencies", &parsed.dependencies),
        ("dev-dependencies", &parsed.dev_dependencies),
        ("build-dependencies", &parsed.build_dependencies),
    ];
    let mut edges = Vec::new();
    for (section, deps) in sections {
        for (key, value) in deps {
            let target = if let Some(p) = path_of(value.get_ref()) {
                module.path.join(p)
            } else if is_workspace_dep(value.get_ref()) {
                match workspace_deps.get(key) {
                    Some(p) => p.clone(),
                    None => continue,
                }
            } else {
                continue;
            };
            let Ok(canonical) = target.canonicalize() else {
                continue;
            };
            if let Some(&to) = by_path.get(canonical.as_path())
                && to != module.name
            {
                edges.push(Edge {
                    from: module.name.clone(),
                    to: to.to_string(),
                    section,
                    span: Span::from_byte_range(file.clone(), &text, value.span()),
                });
            }
        }
    }
    Ok(edges)
}

/// Finds one dependency cycle, rendered as `a -> b -> a`.
fn find_cycle(modules: &[Module], edges: &[Edge]) -> Option<String> {
    let mut adj: BTreeMap<&str, BTreeSet<&str>> = modules
        .iter()
        .map(|m| (m.name.as_str(), BTreeSet::new()))
        .collect();
    for e in edges {
        adj.entry(e.from.as_str())
            .or_default()
            .insert(e.to.as_str());
    }
    let mut state: HashMap<&str, bool> = HashMap::new(); // true = on the current stack, false = finished
    let mut stack: Vec<&str> = Vec::new();
    adj.keys()
        .find_map(|&start| visit(start, &adj, &mut state, &mut stack))
}

fn visit<'a>(
    node: &'a str,
    adj: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    state: &mut HashMap<&'a str, bool>,
    stack: &mut Vec<&'a str>,
) -> Option<String> {
    match state.get(node) {
        Some(true) => {
            let pos = stack.iter().position(|n| *n == node).unwrap_or(0);
            let mut path: Vec<&str> = stack[pos..].to_vec();
            path.push(node);
            return Some(path.join(" -> "));
        }
        Some(false) => return None,
        None => {}
    }
    state.insert(node, true);
    stack.push(node);
    let found = adj
        .get(node)
        .into_iter()
        .flatten()
        .find_map(|&next| visit(next, adj, state, stack));
    stack.pop();
    state.insert(node, false);
    found
}

/// Checks every module's real Cargo dependencies against its manifest.
///
/// Codes: `FA001` dependency not in `allowed`, `FA002` dependency in
/// `forbidden`, `FA003` dependency cycle, `FA004` unreadable `Cargo.toml`.
#[must_use]
pub fn check_architecture(project: &Project) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let workspace_deps = workspace_dependencies(&project.root);
    let by_path: HashMap<&Path, &str> = project
        .modules
        .iter()
        .map(|m| (m.path.as_path(), m.name.as_str()))
        .collect();
    let by_name: HashMap<&str, &Module> = project
        .modules
        .iter()
        .map(|m| (m.name.as_str(), m))
        .collect();

    let mut edges = Vec::new();
    for module in &project.modules {
        match module_edges(module, &workspace_deps, &by_path) {
            Ok(e) => edges.extend(e),
            Err(d) => diags.push(d),
        }
    }

    for edge in &edges {
        let from = by_name[edge.from.as_str()];
        let where_ = format!("{} of {}", edge.section, edge.span.file.display());
        if from.forbidden.contains(&edge.to) {
            diags.push(
                Diagnostic::error(
                    "FA002",
                    format!(
                        "module `{}` depends on forbidden module `{}` ({where_})",
                        edge.from, edge.to
                    ),
                )
                .with_span(edge.span.clone())
                .with_help("remove the dependency or change the design; forbidden means forbidden"),
            );
        } else if !from.allowed.contains(&edge.to) {
            diags.push(
                Diagnostic::error(
                    "FA001",
                    format!("module `{}` depends on `{}` ({where_}) but `{}` is not in its allowed list", edge.from, edge.to, edge.to),
                )
                .with_span(edge.span.clone())
                .with_help(format!("add \"{}\" to [dependencies].allowed in {}/forge.module.toml if the dependency is intended", edge.to, edge.from)),
            );
        }
    }

    if let Some(cycle) = find_cycle(&project.modules, &edges) {
        diags.push(Diagnostic::error(
            "FA003",
            format!("dependency cycle between modules: {cycle}"),
        ));
    }
    diags
}
