use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use walkdir::WalkDir;

use crate::config::{Config, TsConfig};
use crate::models::{ModuleGraph, ParsedModule, Violation};
use crate::parser::TsParser;
use crate::rules::{all_rules, v1_0_rules, LintContext, Rule};
use crate::suppression::SuppressionMap;

/// Top-level linting engine that coordinates file discovery, parsing, rule
/// evaluation, and suppression filtering.
pub struct LintEngine {
    parser: TsParser,
    unstable_rules: bool,
}

impl LintEngine {
    /// Create a new engine backed by a tree-sitter TypeScript parser.
    pub fn new(unstable_rules: bool) -> anyhow::Result<Self> {
        Ok(Self {
            parser: TsParser::new()?,
            unstable_rules,
        })
    }

    /// Resolve a mock source string to an absolute file path.
    ///
    /// Tries relative resolution from the test file's directory, then tsconfig
    /// path aliases. Returns `None` if the target cannot be resolved.
    #[must_use]
    pub fn resolve_mock_target(
        test_path: &Path,
        mock_source: &str,
        tsconfig: Option<&TsConfig>,
    ) -> Option<PathBuf> {
        let exts = [".ts", ".tsx", ".js", ".jsx"];
        let base = test_path.parent()?.join(mock_source);

        // Try with extensions
        for ext in &exts {
            let candidate = base.with_extension(ext.strip_prefix('.').unwrap());
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        // Try index files
        for name in &["index.ts", "index.tsx", "index.js", "index.jsx"] {
            let candidate = base.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        // Try tsconfig paths
        if let Some(ts) = tsconfig {
            let from_dir = test_path.parent().unwrap_or(Path::new("."));
            if let Some(resolved) = ts.resolve(mock_source, from_dir) {
                return Some(resolved);
            }
        }

        None
    }

    /// Lint all test files discovered under `paths` and return sorted violations.
    ///
    /// Two-phase pipeline:
    /// 1. Parallel parse test files + source modules, build ModuleGraph
    /// 2. Parallel rule evaluation per-file with shared ModuleGraph
    pub fn lint_paths(&self, paths: &[PathBuf]) -> anyhow::Result<Vec<Violation>> {
        let files = Self::discover_files(paths);

        // Phase 1: Parse test files in parallel
        let (modules, sources) = self.parse_files(&files);

        // Discover source modules referenced by imports
        let source_modules = Self::discover_source_modules(&modules);

        // Build module graph
        let graph = ModuleGraph::new(&modules, &source_modules);

        // Group modules by their resolved config root
        let groups = Self::group_by_config(&modules);

        let rules = if self.unstable_rules {
            all_rules()
        } else {
            v1_0_rules()
        };

        // Pre-parse suppression maps once per file
        let suppressions = pre_parse_suppressions(&sources);

        // Phase 2: Rule evaluation with shared ModuleGraph
        let violations = evaluate_rules(&groups, &rules, &modules, &suppressions, &graph);

        Ok(violations)
    }

    /// Phase 1: parse test files in parallel, returning modules and their source text.
    fn parse_files(&self, files: &[PathBuf]) -> (Vec<ParsedModule>, Vec<String>) {
        let parsed: Vec<_> = files
            .par_iter()
            .filter_map(|file| {
                let source = std::fs::read_to_string(file).ok()?;
                let module = self.parser.parse_file(file).ok()?;
                Some((module, source))
            })
            .collect();

        let mut modules = Vec::with_capacity(parsed.len());
        let mut sources = Vec::with_capacity(parsed.len());
        for (module, source) in parsed {
            modules.push(module);
            sources.push(source);
        }
        (modules, sources)
    }

    /// Group modules by their resolved config root directory.
    fn group_by_config(modules: &[ParsedModule]) -> HashMap<PathBuf, (Config, Vec<usize>)> {
        let mut groups: HashMap<PathBuf, (Config, Vec<usize>)> = HashMap::new();
        for (idx, module) in modules.iter().enumerate() {
            let config_root = Self::resolve_config_root(&module.file_path);
            let entry = groups.entry(config_root).or_insert_with_key(|root| {
                let config = Config::load_from(root).unwrap_or_default();
                (config, Vec::new())
            });
            entry.1.push(idx);
        }
        groups
    }

    /// Walk up from the module's path to find the directory containing
    /// `.vitest-linter.toml`, falling back to the module's parent directory.
    fn resolve_config_root(path: &Path) -> PathBuf {
        let dir = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        };
        let mut cur = Some(dir.as_path());
        while let Some(d) = cur {
            if d.join(".vitest-linter.toml").is_file() {
                return d.to_path_buf();
            }
            cur = d.parent();
        }
        dir
    }

    fn discover_files(paths: &[PathBuf]) -> Vec<PathBuf> {
        let candidates: Vec<PathBuf> = paths
            .par_iter()
            .flat_map_iter(|path| {
                if path.is_file() {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if is_test_file(&name) {
                        vec![path.clone()]
                    } else {
                        vec![]
                    }
                } else if path.is_dir() {
                    WalkDir::new(path)
                        .into_iter()
                        .filter_map(std::result::Result::ok)
                        .filter(|entry| {
                            let name = entry.file_name().to_string_lossy().to_string();
                            is_test_file(&name)
                        })
                        .map(|entry| entry.into_path())
                        .collect()
                } else {
                    vec![]
                }
            })
            .collect();

        let mut files = candidates;
        files.sort();
        files.dedup();
        files
    }

    /// Discover source modules referenced by imports in test files.
    /// Skips node_modules and external packages.
    fn discover_source_modules(modules: &[ParsedModule]) -> Vec<ParsedModule> {
        let mut source_files: Vec<PathBuf> =
            modules.iter().flat_map(collect_source_paths).collect();

        source_files.sort();
        source_files.dedup();

        // Parse source modules in parallel
        let parser = TsParser::new().ok();
        if let Some(parser) = parser {
            source_files
                .par_iter()
                .filter_map(|file| parser.parse_file(file).ok())
                .collect()
        } else {
            vec![]
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 2: rule evaluation
// ---------------------------------------------------------------------------

/// Evaluate all rules against grouped modules, applying severity overrides and
/// suppression filtering. Returns sorted violations.
fn evaluate_rules(
    groups: &HashMap<PathBuf, (Config, Vec<usize>)>,
    rules: &[Box<dyn Rule>],
    modules: &[ParsedModule],
    suppressions: &[SuppressionMap],
    graph: &ModuleGraph,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    for (config, indices) in groups.values() {
        let group_modules: Vec<ParsedModule> =
            indices.iter().map(|i| modules[*i].clone()).collect();
        let ctx = LintContext {
            config,
            all_modules: &group_modules,
        };
        for rule in rules {
            if config.rules.is_disabled(rule.id()) {
                continue;
            }
            for (local_idx, module) in group_modules.iter().enumerate() {
                if !rule.applies_to_runtime(module.runtime) {
                    continue;
                }
                let global_idx = indices[local_idx];
                let mut v = rule.check(module, &ctx, graph);
                apply_severity_overrides(&mut v, config, rule.id());
                let suppression = &suppressions[global_idx];
                v.retain(|violation| {
                    !suppression.is_suppressed(violation.line, &violation.rule_id)
                });
                violations.append(&mut v);
            }
        }
    }

    violations.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then_with(|| a.line.cmp(&b.line))
    });
    violations
}

/// Apply severity overrides from config to a list of violations.
fn apply_severity_overrides(v: &mut Vec<Violation>, config: &Config, rule_id: &str) {
    let Some(override_sev) = config.rules.severity_override(rule_id) else {
        return;
    };
    for violation in &mut *v {
        violation.severity = match override_sev.to_ascii_lowercase().as_str() {
            "error" => crate::models::Severity::Error,
            "warning" => crate::models::Severity::Warning,
            "info" => crate::models::Severity::Info,
            _ => violation.severity,
        };
    }
}

// ---------------------------------------------------------------------------
// Suppression helpers
// ---------------------------------------------------------------------------

/// Pre-parse suppression comment maps from source text.
fn pre_parse_suppressions(sources: &[String]) -> Vec<SuppressionMap> {
    sources.iter().map(|s| SuppressionMap::parse(s)).collect()
}

// ---------------------------------------------------------------------------
// Source module discovery helpers
// ---------------------------------------------------------------------------

/// Collect all resolved source file paths referenced by a module's imports and
/// vi.mock() calls, skipping external packages.
fn collect_source_paths(module: &ParsedModule) -> Vec<PathBuf> {
    let mut sources: Vec<&str> = Vec::new();
    for imp in &module.imports_parsed {
        sources.push(&imp.source);
    }
    for mock in &module.vi_mocks {
        if !sources.contains(&mock.source.as_str()) {
            sources.push(&mock.source);
        }
    }

    sources
        .iter()
        .filter_map(|source| resolve_source_path(module, source))
        .collect()
}

/// Resolve a single import source string to an absolute file path.
///
/// Attempts extension-less resolution, then index file resolution, within the
/// module's parent directory. Returns `None` for external packages or
/// unresolvable paths.
fn resolve_source_path(module: &ParsedModule, source: &str) -> Option<PathBuf> {
    // Skip external packages
    if !source.starts_with('.') && !source.starts_with('/') {
        return None;
    }
    let parent = module.file_path.parent()?;
    let base = parent.join(source);
    let exts = [".ts", ".tsx", ".js", ".jsx"];

    for ext in &exts {
        let candidate = base.with_extension(ext.strip_prefix('.').unwrap());
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    for name in &["index.ts", "index.tsx", "index.js", "index.jsx"] {
        let candidate = base.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_test_file(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(".test.ts")
        || name.ends_with(".spec.ts")
        || name.ends_with(".test.tsx")
        || name.ends_with(".spec.tsx")
        || name.ends_with(".test.js")
        || name.ends_with(".spec.js")
        || name.ends_with(".test.jsx")
        || name.ends_with(".spec.jsx")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_accepted_extensions() {
        assert!(is_test_file("foo.test.ts"));
        assert!(is_test_file("foo.spec.ts"));
        assert!(is_test_file("foo.test.tsx"));
        assert!(is_test_file("foo.spec.tsx"));
        assert!(is_test_file("foo.test.js"));
        assert!(is_test_file("foo.spec.js"));
        assert!(is_test_file("foo.test.jsx"));
        assert!(is_test_file("foo.spec.jsx"));
        assert!(is_test_file("path/to/bar.test.ts"));
        assert!(is_test_file("path/to/bar.spec.js"));
        assert!(is_test_file("path/to/baz.test.tsx"));
        // Case-insensitive matching
        assert!(is_test_file("Foo.TEST.ts"));
        assert!(is_test_file("path/To/BaZ.Spec.JS"));
        assert!(is_test_file("UPPER.TEST.TSX"));
    }

    #[test]
    fn test_file_rejected_extensions() {
        assert!(!is_test_file("foo.ts"));
        assert!(!is_test_file("foo.js"));
        assert!(!is_test_file("foo.tsx"));
        assert!(!is_test_file("foo.jsx"));
        assert!(!is_test_file("utils.ts"));
        assert!(!is_test_file("README.md"));
        assert!(!is_test_file("foo.test.py"));
        assert!(!is_test_file("test.ts"));
    }

    #[test]
    fn resolve_mock_target_relative() {
        let dir = tempfile::TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let src = src_dir.join("utils.ts");
        std::fs::write(&src, "export const foo = 1;").unwrap();
        let test = src_dir.join("utils.test.ts");

        let resolved = LintEngine::resolve_mock_target(&test, "./utils", None);
        assert_eq!(resolved, Some(src));
    }

    #[test]
    fn resolve_mock_target_unresolvable() {
        let dir = tempfile::TempDir::new().unwrap();
        let test = dir.path().join("test.ts");

        let resolved = LintEngine::resolve_mock_target(&test, "./nonexistent", None);
        assert_eq!(resolved, None);
    }
}
