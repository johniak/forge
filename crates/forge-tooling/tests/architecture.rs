//! Architecture guard: real Cargo dependencies vs module manifests (ADR-0005).
#![allow(clippy::unwrap_used)]

mod common;

use common::{TempProject, valid_project};
use forge_tooling::architecture::check_architecture;
use forge_tooling::manifest::load_project;
use forge_tooling::report::Diagnostic;

fn codes(diags: &[Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.as_str()).collect()
}

#[test]
fn allowed_path_dependency_passes() {
    let tp = valid_project();
    let project = load_project(tp.root()).unwrap();
    assert!(check_architecture(&project).is_empty());
}

#[test]
fn unlisted_dependency_is_fa001_with_span_on_the_dependency_entry() {
    let tp = valid_project();
    tp.module(
        "cli",
        "crates/cli",
        &[],
        &[],
        "core = { path = \"../core\" }",
    );
    let project = load_project(tp.root()).unwrap();
    let diags = check_architecture(&project);
    assert_eq!(codes(&diags), vec!["FA001"]);
    assert!(
        diags[0].message.contains("cli") && diags[0].message.contains("core"),
        "{}",
        diags[0].message
    );
    let span = diags[0].span.as_ref().unwrap();
    assert!(span.file.ends_with("crates/cli/Cargo.toml"));
    assert_eq!(span.line, 7);
}

#[test]
fn forbidden_dependency_is_fa002() {
    let tp = valid_project();
    tp.module(
        "core",
        "crates/core",
        &[],
        &["cli"],
        "cli = { path = \"../cli\" }",
    );
    tp.module("cli", "crates/cli", &["core"], &[], "");
    let project = load_project(tp.root()).unwrap();
    assert_eq!(codes(&check_architecture(&project)), vec!["FA002"]);
}

#[test]
fn workspace_dependency_is_resolved_through_root_cargo_toml() {
    let tp = valid_project();
    tp.write(
        "Cargo.toml",
        "[workspace]\nresolver = \"3\"\nmembers = [\"crates/core\", \"crates/cli\"]\n\n[workspace.dependencies]\ncore = { path = \"crates/core\" }\nserde = \"1\"\n",
    );
    tp.module(
        "cli",
        "crates/cli",
        &[],
        &[],
        "core.workspace = true\nserde = { workspace = true }",
    );
    let project = load_project(tp.root()).unwrap();
    let diags = check_architecture(&project);
    assert_eq!(codes(&diags), vec!["FA001"]);
    assert!(
        diags[0]
            .span
            .as_ref()
            .unwrap()
            .file
            .ends_with("crates/cli/Cargo.toml")
    );
}

#[test]
fn dev_and_build_dependencies_are_checked_too() {
    let tp = valid_project();
    tp.module("cli", "crates/cli", &[], &[], "");
    tp.write(
        "crates/cli/Cargo.toml",
        "[package]\nname = \"cli\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dev-dependencies]\ncore = { path = \"../core\" }\n\n[build-dependencies]\ncore = { path = \"../core\" }\n",
    );
    let project = load_project(tp.root()).unwrap();
    assert_eq!(codes(&check_architecture(&project)), vec!["FA001", "FA001"]);
}

#[test]
fn dependencies_outside_the_project_are_ignored() {
    let tp = valid_project();
    tp.module("cli", "crates/cli", &["core"], &[], "core = { path = \"../core\" }\nserde = \"1\"\nother = { path = \"../../elsewhere\" }\nrenamed = { package = \"serde_json\", version = \"1\" }");
    let project = load_project(tp.root()).unwrap();
    assert!(check_architecture(&project).is_empty());
}

#[test]
fn renamed_path_dependency_is_matched_by_path_not_by_key() {
    let tp = valid_project();
    tp.module(
        "cli",
        "crates/cli",
        &[],
        &[],
        "kernel = { path = \"../core\", package = \"core\" }",
    );
    let project = load_project(tp.root()).unwrap();
    let diags = check_architecture(&project);
    assert_eq!(codes(&diags), vec!["FA001"]);
    assert!(diags[0].message.contains("core"));
}

#[test]
fn dependency_cycle_is_fa003() {
    let tp = TempProject::new();
    tp.write("Cargo.toml", "[workspace]\nmembers = [\"a\", \"b\"]\n");
    tp.forge_toml(&[("a", "a"), ("b", "b")]);
    tp.module("a", "a", &["b"], &[], "b = { path = \"../b\" }");
    tp.module("b", "b", &["a"], &[], "a = { path = \"../a\" }");
    let project = load_project(tp.root()).unwrap();
    let diags = check_architecture(&project);
    assert_eq!(codes(&diags), vec!["FA003"]);
    assert!(
        diags[0].message.contains("a -> b -> a") || diags[0].message.contains("b -> a -> b"),
        "{}",
        diags[0].message
    );
}

#[test]
fn module_without_cargo_toml_is_fa004() {
    let tp = valid_project();
    std::fs::remove_file(tp.root().join("crates/core/Cargo.toml")).unwrap();
    let project = load_project(tp.root()).unwrap();
    assert_eq!(codes(&check_architecture(&project)), vec!["FA004"]);
}

#[test]
fn unparseable_cargo_toml_is_fa004_with_span() {
    let tp = valid_project();
    tp.write("crates/core/Cargo.toml", "[package\n");
    let project = load_project(tp.root()).unwrap();
    let diags = check_architecture(&project);
    assert_eq!(codes(&diags), vec!["FA004"]);
    assert_eq!(diags[0].span.as_ref().unwrap().line, 1);
}

#[test]
fn missing_workspace_cargo_toml_is_fa004_and_modules_are_still_checked() {
    // Without the root Cargo.toml, `workspace = true` dependencies cannot be
    // resolved; the guard must say so instead of passing.
    let tp = valid_project();
    std::fs::remove_file(tp.root().join("Cargo.toml")).unwrap();
    tp.write(
        "crates/core/Cargo.toml",
        "[package]\nname = \"core\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\ncli = { path = \"../cli\" }\n",
    );
    let project = load_project(tp.root()).unwrap();
    let diags = check_architecture(&project);
    assert_eq!(codes(&diags), vec!["FA004", "FA002", "FA003"], "{diags:?}");
    assert!(diags[0].message.contains("Cargo.toml"));
}

#[test]
fn unparseable_workspace_cargo_toml_is_fa004_with_span() {
    let tp = valid_project();
    tp.write("Cargo.toml", "[workspace\n");
    let project = load_project(tp.root()).unwrap();
    let diags = check_architecture(&project);
    assert_eq!(codes(&diags), vec!["FA004"]);
    let span = diags[0].span.as_ref().unwrap();
    assert!(span.file.ends_with("Cargo.toml"));
    assert_eq!(span.line, 1);
}

#[test]
fn path_dependency_on_a_non_module_crate_is_ignored() {
    let tp = valid_project();
    tp.write(
        "vendor/helper/Cargo.toml",
        "[package]\nname = \"helper\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    );
    tp.write(
        "crates/core/Cargo.toml",
        "[package]\nname = \"core\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nhelper = { path = \"../../vendor/helper\" }\n",
    );
    let project = load_project(tp.root()).unwrap();
    assert!(check_architecture(&project).is_empty());
}

#[test]
fn self_path_dependency_is_not_an_edge() {
    let tp = valid_project();
    tp.write(
        "crates/core/Cargo.toml",
        "[package]\nname = \"core\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dev-dependencies]\ncore = { path = \".\" }\n",
    );
    let project = load_project(tp.root()).unwrap();
    assert!(check_architecture(&project).is_empty());
}

#[test]
fn registry_dependency_sharing_a_workspace_name_is_not_an_edge() {
    // Only `workspace = true` resolves through [workspace.dependencies]; a
    // registry dependency that happens to share the name is not a module edge.
    let tp = valid_project();
    tp.write(
        "Cargo.toml",
        "[workspace]\nresolver = \"3\"\nmembers = [\"crates/core\", \"crates/cli\"]\n\n[workspace.dependencies]\ncore = { path = \"crates/core\" }\n",
    );
    tp.module("cli", "crates/cli", &[], &[], "core = \"1.0\"");
    let project = load_project(tp.root()).unwrap();
    assert!(check_architecture(&project).is_empty());
}

#[test]
fn target_specific_dependencies_are_checked_too() {
    // `[target.'cfg(…)'.dependencies]` is as real an edge as `[dependencies]`.
    let tp = valid_project();
    tp.write(
        "crates/core/Cargo.toml",
        "[package]\nname = \"core\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[target.'cfg(all())'.dependencies]\ncli = { path = \"../cli\" }\n\n[target.'cfg(unix)'.dev-dependencies]\ncli = { path = \"../cli\" }\n",
    );
    let project = load_project(tp.root()).unwrap();
    let diags = check_architecture(&project);
    let fa002: Vec<&Diagnostic> = diags.iter().filter(|d| d.code == "FA002").collect();
    assert_eq!(fa002.len(), 2, "{diags:?}");
    assert!(
        fa002[0]
            .message
            .contains("target.'cfg(all())'.dependencies")
            || fa002[0].message.contains("cfg(all())"),
        "{}",
        fa002[0].message
    );
    assert_eq!(fa002[0].span.as_ref().unwrap().line, 7);
}
