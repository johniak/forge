//! `forge` CLI: a thin adapter over `forge-tooling`.
//!
//! Exit codes: `0` PASS, `1` FAIL, `3` `NOT_VERIFIED`, `2` usage error.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use forge_tooling::check::{CheckId, CheckOptions, UnknownCheck, run_check};
use forge_tooling::process::SystemRunner;
use forge_tooling::report::{CheckReport, Status};

#[derive(Parser)]
#[command(name = "forge", version, about = "Forge developer tooling")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validates manifests, architecture boundaries, formatting and lints.
    Check(CheckArgs),
}

#[derive(Args)]
struct CheckArgs {
    /// Project root (the directory holding forge.toml).
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
    /// Checks to skip; each is reported as `NOT_VERIFIED` (manifests, architecture, fmt, clippy).
    #[arg(long = "skip", value_name = "CHECK", value_parser = parse_check_id)]
    skip: Vec<CheckId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    /// Compact human-readable text.
    Text,
    /// Versioned JSON report (`schema_version` 1).
    Json,
}

fn parse_check_id(s: &str) -> Result<CheckId, String> {
    s.parse().map_err(|e: UnknownCheck| e.to_string())
}

fn render(report: &CheckReport, format: Format) -> String {
    match format {
        Format::Text => report.render_text(),
        Format::Json => serde_json::to_string_pretty(report).unwrap_or_default() + "\n",
    }
}

fn exit_code(status: Status) -> ExitCode {
    match status {
        Status::Pass => ExitCode::SUCCESS,
        Status::Fail => ExitCode::from(1),
        Status::NotVerified => ExitCode::from(3),
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check(args) => {
            let report = run_check(
                &CheckOptions {
                    root: args.root,
                    skip: args.skip,
                },
                &SystemRunner,
            );
            print!("{}", render(&report, args.format));
            exit_code(report.status)
        }
    }
}
