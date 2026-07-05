pub mod config;
pub mod engine;
pub mod models;
pub mod parser;
pub mod rules;
pub mod suppression;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use colored::Colorize;

use engine::LintEngine;
use models::Severity;

fn get_changed_files(base: &str) -> Result<Vec<PathBuf>> {
    let output = std::process::Command::new("git")
        .args(["diff", "--name-only", "--diff-filter=ACMR", base])
        .output()?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<PathBuf> = stdout
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.ends_with(".test.ts")
                || lower.ends_with(".spec.ts")
                || lower.ends_with(".test.tsx")
                || lower.ends_with(".spec.tsx")
                || lower.ends_with(".test.js")
                || lower.ends_with(".spec.js")
                || lower.ends_with(".test.jsx")
                || lower.ends_with(".spec.jsx")
        })
        .map(PathBuf::from)
        .collect();

    Ok(files)
}

/// Run the CLI with the given arguments and return whether error-severity
/// violations were found.
pub fn run_cli(
    paths: &[PathBuf],
    format: &str,
    output: Option<&Path>,
    no_color: bool,
    incremental: bool,
    unstable_rules: bool,
    base: &str,
) -> Result<bool> {
    if no_color {
        colored::control::set_override(false);
    }

    let effective_paths = if incremental {
        let changed = get_changed_files(base)?;
        if changed.is_empty() {
            if format == "json" {
                println!("[]");
            } else {
                println!("No changed test files detected.");
            }
            return Ok(false);
        }
        changed
    } else {
        paths.to_vec()
    };

    let engine = LintEngine::new(unstable_rules)?;
    let violations = engine.lint_paths(&effective_paths)?;

    match format {
        "json" => output_json(&violations, output)?,
        "sarif" => output_sarif(&violations, output)?,
        _ => output_human(&violations, output)?,
    }

    let has_errors = violations.iter().any(|v| v.severity == Severity::Error);
    Ok(has_errors)
}

fn output_json(violations: &[models::Violation], output: Option<&Path>) -> Result<()> {
    let json = serde_json::to_string_pretty(violations)?;
    match output {
        Some(path) => fs::write(path, json)?,
        None => println!("{json}"),
    }
    Ok(())
}

fn output_sarif(violations: &[models::Violation], output: Option<&Path>) -> Result<()> {
    let sarif = build_sarif(violations);
    let json = serde_json::to_string_pretty(&sarif)?;
    match output {
        Some(path) => fs::write(path, json)?,
        None => println!("{json}"),
    }
    Ok(())
}

fn output_human(violations: &[models::Violation], output: Option<&Path>) -> Result<()> {
    let mut out: Box<dyn Write> = match output {
        Some(path) => Box::new(fs::File::create(path)?),
        None => Box::new(std::io::stdout()),
    };

    if violations.is_empty() {
        writeln!(out, "{} No test smells detected.", "\u{2713}".green())?;
    } else {
        for v in violations {
            let severity_str = match v.severity {
                Severity::Error => "Error".red().bold().to_string(),
                Severity::Warning => "Warning".yellow().bold().to_string(),
                Severity::Info => "Info".blue().bold().to_string(),
            };
            writeln!(
                out,
                "{}: {} in {}:{}",
                severity_str,
                v.rule_id.cyan(),
                v.file_path.display().to_string().white(),
                v.line
            )?;
            writeln!(out, "  {}", v.message)?;
            if let Some(ref suggestion) = v.suggestion {
                writeln!(out, "  {} {}", "Suggestion:".dimmed(), suggestion.dimmed())?;
            }
            writeln!(out)?;
        }

        let errors = violations
            .iter()
            .filter(|v| v.severity == Severity::Error)
            .count();
        let warnings = violations
            .iter()
            .filter(|v| v.severity == Severity::Warning)
            .count();
        let infos = violations
            .iter()
            .filter(|v| v.severity == Severity::Info)
            .count();

        writeln!(
            out,
            "Found {} violation(s): {} error(s), {} warning(s), {} info",
            violations.len(),
            errors,
            warnings,
            infos
        )?;
    }

    Ok(())
}

fn build_sarif(violations: &[models::Violation]) -> serde_json::Value {
    let results: Vec<serde_json::Value> = violations
        .iter()
        .map(|v| {
            let level = match v.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "note",
            };
            serde_json::json!({
                "ruleId": v.rule_id,
                "level": level,
                "message": { "text": v.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {
                            "uri": v.file_path.display().to_string()
                        },
                        "region": {
                            "startLine": v.line
                        }
                    }
                }],
                "properties": {
                    "ruleName": v.rule_name,
                    "category": format!("{:?}", v.category),
                    "suggestion": v.suggestion,
                    "testName": v.test_name
                }
            })
        })
        .collect();

    serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "vitest-linter",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/VidiomTM/vitest-linter",
                    "rules": violations.iter().map(|v| serde_json::json!({
                        "id": v.rule_id,
                        "name": v.rule_name,
                        "shortDescription": { "text": v.message },
                        "properties": {
                            "category": format!("{:?}", v.category)
                        }
                    })).collect::<Vec<_>>()
                }
            },
            "results": results
        }]
    })
}
