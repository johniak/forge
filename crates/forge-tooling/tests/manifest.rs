//! `forge.toml` / `forge.module.toml` loading and validation (ADR-0005).
#![allow(clippy::unwrap_used)]

mod common;

use common::{TempProject, valid_project};
use forge_tooling::manifest::{ModuleKind, load_project};
use forge_tooling::report::{Diagnostic, Severity};

fn codes(diags: &[Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.as_str()).collect()
}

#[test]
fn loads_a_valid_project() {
    let tp = valid_project();
    let project = load_project(tp.root()).unwrap();
    assert_eq!(project.name, "demo");
    assert_eq!(project.root, tp.root().canonicalize().unwrap());
    assert_eq!(project.modules.len(), 2);
    let cli = project.modules.iter().find(|m| m.name == "cli").unwrap();
    assert_eq!(cli.kind, ModuleKind::Domain);
    assert_eq!(
        cli.path,
        tp.root().canonicalize().unwrap().join("crates/cli")
    );
    assert_eq!(cli.allowed, vec!["core".to_string()]);
    assert!(cli.forbidden.is_empty());
    assert_eq!(cli.verification.line_coverage, 100);
    assert_eq!(cli.verification.branch_coverage, 100);
    let core = project.modules.iter().find(|m| m.name == "core").unwrap();
    assert_eq!(core.forbidden, vec!["cli".to_string()]);
}

#[test]
fn missing_forge_toml_is_fm001_without_span() {
    let tp = TempProject::new();
    let diags = load_project(tp.root()).unwrap_err();
    assert_eq!(codes(&diags), vec!["FM001"]);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(diags[0].span.is_none());
    assert!(
        diags[0].message.contains("forge.toml"),
        "{}",
        diags[0].message
    );
}

#[test]
fn syntax_error_in_forge_toml_has_a_span() {
    let tp = TempProject::new();
    tp.write(
        "forge.toml",
        "[project]\nname = \"demo\"\nmanifest_version = 1\n\n[modules]\ncore = \n",
    );
    let diags = load_project(tp.root()).unwrap_err();
    assert_eq!(codes(&diags), vec!["FM001"]);
    let span = diags[0].span.as_ref().unwrap();
    assert!(span.file.ends_with("forge.toml"));
    assert_eq!(span.line, 6, "span: {span:?}");
    assert!(span.column >= 1);
}

#[test]
fn unknown_module_kind_is_a_parse_error_with_span() {
    let tp = valid_project();
    tp.write(
        "crates/core/forge.module.toml",
        "[module]\nname = \"core\"\nkind = \"spaceship\"\ndescription = \"x\"\n",
    );
    let diags = load_project(tp.root()).unwrap_err();
    assert_eq!(codes(&diags), vec!["FM001"]);
    let span = diags[0].span.as_ref().unwrap();
    assert!(span.file.ends_with("crates/core/forge.module.toml"));
    assert_eq!(span.line, 3);
}

#[test]
fn module_directory_without_manifest_is_fm002_pointing_at_forge_toml_entry() {
    let tp = valid_project();
    std::fs::remove_file(tp.root().join("crates/cli/forge.module.toml")).unwrap();
    let diags = load_project(tp.root()).unwrap_err();
    assert_eq!(codes(&diags), vec!["FM002"]);
    let span = diags[0].span.as_ref().unwrap();
    assert!(span.file.ends_with("forge.toml"));
    assert_eq!(span.line, 7, "the `cli = ...` line");
}

#[test]
fn module_name_mismatch_is_fm003_pointing_at_module_manifest() {
    let tp = valid_project();
    tp.write(
        "crates/core/forge.module.toml",
        "[module]\nname = \"kore\"\nkind = \"domain\"\ndescription = \"x\"\n",
    );
    let diags = load_project(tp.root()).unwrap_err();
    assert_eq!(codes(&diags), vec!["FM003"]);
    let span = diags[0].span.as_ref().unwrap();
    assert!(span.file.ends_with("crates/core/forge.module.toml"));
    assert_eq!(span.line, 2);
}

#[test]
fn two_modules_sharing_a_path_is_fm004() {
    let tp = valid_project();
    tp.forge_toml(&[
        ("core", "crates/core"),
        ("cli", "crates/cli"),
        ("core2", "crates/core"),
    ]);
    let diags = load_project(tp.root()).unwrap_err();
    assert!(codes(&diags).contains(&"FM004"), "{diags:?}");
}

#[test]
fn allowed_or_forbidden_naming_unknown_module_is_fm005_with_span() {
    let tp = valid_project();
    tp.module(
        "cli",
        "crates/cli",
        &["core", "ghost"],
        &["phantom"],
        "core = { path = \"../core\" }",
    );
    let diags = load_project(tp.root()).unwrap_err();
    assert_eq!(codes(&diags), vec!["FM005", "FM005"]);
    let span = diags[0].span.as_ref().unwrap();
    assert!(span.file.ends_with("crates/cli/forge.module.toml"));
    assert_eq!(span.line, 7, "allowed = [...] line");
    assert!(diags[0].message.contains("ghost"));
    assert!(diags[1].message.contains("phantom"));
}

#[test]
fn module_both_allowed_and_forbidden_is_fm006() {
    let tp = valid_project();
    tp.module(
        "cli",
        "crates/cli",
        &["core"],
        &["core"],
        "core = { path = \"../core\" }",
    );
    let diags = load_project(tp.root()).unwrap_err();
    assert_eq!(codes(&diags), vec!["FM006"]);
}

#[test]
fn module_allowing_itself_is_fm005() {
    let tp = valid_project();
    tp.module("core", "crates/core", &["core"], &[], "");
    let diags = load_project(tp.root()).unwrap_err();
    assert_eq!(codes(&diags), vec!["FM005"]);
}

#[test]
fn all_problems_are_reported_together() {
    let tp = valid_project();
    tp.forge_toml(&[
        ("core", "crates/core"),
        ("cli", "crates/cli"),
        ("missing", "crates/missing"),
    ]);
    tp.module("cli", "crates/cli", &["core", "ghost"], &[], "");
    let diags = load_project(tp.root()).unwrap_err();
    let mut c = codes(&diags);
    c.sort_unstable();
    assert_eq!(c, vec!["FM002", "FM005"]);
}

#[test]
fn verification_defaults_to_100_percent_when_omitted() {
    let tp = valid_project();
    tp.write(
        "crates/core/forge.module.toml",
        "[module]\nname = \"core\"\nkind = \"tooling\"\ndescription = \"x\"\n",
    );
    let project = load_project(tp.root()).unwrap();
    let core = project.modules.iter().find(|m| m.name == "core").unwrap();
    assert_eq!(core.kind, ModuleKind::Tooling);
    assert_eq!(core.verification.line_coverage, 100);
    assert_eq!(core.verification.branch_coverage, 100);
    assert!(core.allowed.is_empty());
}

#[test]
fn unsupported_manifest_version_is_fm007_with_span() {
    let tp = valid_project();
    tp.write(
        "forge.toml",
        "[project]\nname = \"demo\"\nmanifest_version = 2\n\n[modules]\ncore = \"crates/core\"\ncli = \"crates/cli\"\n",
    );
    let diags = load_project(tp.root()).unwrap_err();
    assert_eq!(codes(&diags), vec!["FM007"]);
    assert!(diags[0].message.contains('2'));
    let span = diags[0].span.as_ref().unwrap();
    assert!(span.file.ends_with("forge.toml"));
    assert_eq!(span.line, 3);
}

#[test]
fn module_forbidding_itself_is_fm005() {
    let tp = valid_project();
    tp.module("core", "crates/core", &[], &["core"], "");
    let diags = load_project(tp.root()).unwrap_err();
    assert_eq!(codes(&diags), vec!["FM005"]);
    assert!(diags[0].message.contains("forbidden"));
}

#[test]
fn testing_kind_is_accepted() {
    let tp = valid_project();
    tp.write(
        "crates/core/forge.module.toml",
        "[module]\nname = \"core\"\nkind = \"testing\"\n\n[dependencies]\nforbidden = [\"cli\"]\n",
    );
    let project = load_project(tp.root()).unwrap();
    let core = project.modules.iter().find(|m| m.name == "core").unwrap();
    assert_eq!(core.kind, ModuleKind::Testing);
}

#[test]
fn coverage_threshold_above_100_is_fm008() {
    let tp = valid_project();
    tp.write(
        "crates/core/forge.module.toml",
        "[module]\nname = \"core\"\nkind = \"domain\"\n\n[dependencies]\nforbidden = [\"cli\"]\n\n[verification]\nbranch_coverage = 101\n",
    );
    let diags = load_project(tp.root()).unwrap_err();
    assert_eq!(codes(&diags), vec!["FM008"]);
    assert!(diags[0].message.contains("101"));
    assert!(
        diags[0]
            .span
            .as_ref()
            .unwrap()
            .file
            .ends_with("crates/core/forge.module.toml")
    );
}
