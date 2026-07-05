use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Severity level for a lint violation, ordered Error > Warning > Info.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// Category grouping for lint rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Category {
    Flakiness,
    Maintenance,
    Structure,
    Dependencies,
    Validation,
    Playwright,
}

/// Runtime that the test file targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum TestRuntime {
    #[default]
    Unknown,
    Vitest,
    Playwright,
}

#[derive(Debug, Clone, Default)]
pub struct PlaywrightCall {
    pub call_name: String,
    pub line: usize,
    pub raw_arg: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LocatorChain {
    pub root: String,
    pub raw_arg: Option<String>,
    pub method: String,
    pub line: usize,
}

#[derive(Debug, Clone, Default)]
pub struct PlaywrightModule {
    pub calls: Vec<PlaywrightCall>,
    pub locator_chains: Vec<LocatorChain>,
    pub evaluate_inner_text: Vec<usize>,
    pub uses_axe: bool,
}

#[derive(Debug, Clone)]
pub struct GlobalStub {
    pub target: String,
    pub line: usize,
}

/// A single lint violation found by a rule.
#[derive(Debug, Clone, Serialize)]
pub struct Violation {
    pub rule_id: String,
    pub rule_name: String,
    pub severity: Severity,
    pub category: Category,
    pub message: String,
    pub file_path: PathBuf,
    pub line: usize,
    pub col: Option<usize>,
    pub suggestion: Option<String>,
    pub test_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ordering() {
        assert!(Severity::Error < Severity::Warning);
        assert!(Severity::Warning < Severity::Info);
        assert!(Severity::Error < Severity::Info);
    }

    #[test]
    fn severity_equality() {
        assert_eq!(Severity::Error, Severity::Error);
        assert_ne!(Severity::Error, Severity::Warning);
    }

    #[test]
    fn category_values() {
        assert_ne!(Category::Flakiness, Category::Maintenance);
        assert_ne!(Category::Maintenance, Category::Structure);
        assert_ne!(Category::Flakiness, Category::Structure);
        assert_ne!(Category::Playwright, Category::Maintenance);
    }

    #[test]
    fn test_runtime_values() {
        assert_ne!(TestRuntime::Vitest, TestRuntime::Playwright);
        assert_ne!(TestRuntime::Playwright, TestRuntime::Unknown);
        assert_ne!(TestRuntime::Vitest, TestRuntime::Unknown);
    }
}

#[derive(Debug, Clone)]
pub struct TestBlock {
    pub name: String,
    pub file_path: PathBuf,
    pub line: usize,
    pub has_assertions: bool,
    pub assertion_count: usize,
    pub has_conditional_logic: bool,
    pub has_try_catch: bool,
    pub uses_settimeout: bool,
    pub uses_promise_settimeout: bool,
    pub uses_datemock: bool,
    pub has_multiple_expects: bool,
    pub is_skipped: bool,
    pub is_only: bool,
    pub is_nested: bool,
    pub has_return_statement: bool,
    pub unawaited_async_assertions: usize,
    pub uses_fake_timers: bool,
    pub uses_random: bool,
    pub has_expect_call_without_assertion: bool,
    pub has_return_of_expect: bool,
    pub title_is_template_literal: bool,
    pub has_async_expect_wrapper: bool,
    pub uses_fit_or_xit: bool,
    pub has_done_callback: bool,
    pub has_conditional_expect: bool,
    pub weak_assertion_count: usize,
    pub has_real_timers_call: bool,
}

#[derive(Debug, Clone)]
pub struct DescribeBlock {
    pub name: String,
    pub file_path: PathBuf,
    pub line: usize,
    pub is_only: bool,
    pub depth: usize,
    pub title_is_template_literal: bool,
    pub title_is_empty: bool,
    pub is_async: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedModule {
    pub file_path: PathBuf,
    pub imports: Vec<String>,
    pub imports_parsed: Vec<ImportEntry>,
    pub vi_mocks: Vec<ViMockCall>,
    pub hook_calls: Vec<HookCall>,
    pub test_blocks: Vec<TestBlock>,
    pub describe_blocks: Vec<DescribeBlock>,
    pub has_fake_timers: bool,
    pub expects_outside_tests: Vec<ExpectOutsideTest>,
    pub imports_node_test: bool,
    pub snapshot_sizes: Vec<SnapshotSize>,
    pub exports: Vec<ExportEntry>,
    pub runtime: TestRuntime,
    pub playwright: Option<PlaywrightModule>,
    pub global_stubs: Vec<GlobalStub>,
}

#[derive(Debug, Clone)]
pub struct ExpectOutsideTest {
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct SnapshotSize {
    pub line: usize,
    pub size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportKind {
    Named,
    Default,
    Namespace,
}

#[derive(Debug, Clone)]
pub struct ExportEntry {
    pub name: String,
    pub kind: ExportKind,
    pub line: usize,
}

#[derive(Debug, Default)]
pub struct ModuleGraph {
    pub modules: HashMap<PathBuf, ParsedModule>,
    pub edges: HashMap<PathBuf, Vec<PathBuf>>,
}

impl ModuleGraph {
    /// Build a module graph from test modules and source modules.
    #[must_use]
    pub fn new(test_modules: &[ParsedModule], source_modules: &[ParsedModule]) -> Self {
        let mut modules = HashMap::new();
        for module in test_modules.iter().chain(source_modules.iter()) {
            modules.insert(module.file_path.clone(), module.clone());
        }
        let edges = build_edges(&modules, test_modules, source_modules);
        Self { modules, edges }
    }

    /// Get a module by its file path.
    #[must_use]
    pub fn get_module(&self, path: &Path) -> Option<&ParsedModule> {
        self.modules.get(path)
    }

    /// Get the dependencies of a module.
    #[must_use]
    pub fn get_dependencies(&self, path: &Path) -> Vec<&ParsedModule> {
        self.edges
            .get(path)
            .map(|deps| {
                deps.iter()
                    .filter_map(|dep| self.modules.get(dep))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Resolve a relative import to an absolute module path, if the module exists.
fn resolve_import_edge(
    module: &ParsedModule,
    imp: &ImportEntry,
    modules: &HashMap<PathBuf, ParsedModule>,
) -> Option<PathBuf> {
    // Skip non-relative imports (bare specifiers like "vitest", "lodash")
    if !imp.source.starts_with('.') && !imp.source.starts_with('/') {
        return None;
    }

    let parent = module.file_path.parent()?;
    let base = parent.join(&imp.source);
    let exts = [".ts", ".tsx", ".js", ".jsx"];

    // Try direct file match (e.g., "./foo" → "./foo.ts")
    for ext in &exts {
        let candidate = base.with_extension(ext.strip_prefix('.').unwrap());
        if modules.contains_key(&candidate) {
            return Some(candidate);
        }
    }

    // Try index file match (e.g., "./dir" → "./dir/index.ts")
    for ext in &exts {
        let candidate = base.join(format!("index{}", ext));
        if modules.contains_key(&candidate) {
            return Some(candidate);
        }
    }

    None
}

/// Build edges from import statements for all modules.
fn build_edges(
    modules: &HashMap<PathBuf, ParsedModule>,
    test_modules: &[ParsedModule],
    source_modules: &[ParsedModule],
) -> HashMap<PathBuf, Vec<PathBuf>> {
    let mut edges: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    for module in test_modules.iter().chain(source_modules.iter()) {
        let file_path = &module.file_path;
        edges.entry(file_path.clone()).or_default();
        for imp in &module.imports_parsed {
            if let Some(resolved) = resolve_import_edge(module, imp, modules) {
                edges.entry(file_path.clone()).or_default().push(resolved);
            }
        }
    }

    edges
}

#[derive(Debug, Clone)]
pub struct ImportEntry {
    pub source: String,
    pub named: Vec<String>,
    pub default: Option<String>,
    pub namespace: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockScope {
    Module,
    Hook,
    Test,
}

#[derive(Debug, Clone)]
pub struct ViMockCall {
    pub source: String,
    pub line: usize,
    pub scope: MockScope,
    pub factory_keys: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookKind {
    BeforeEach,
    AfterEach,
    BeforeAll,
    AfterAll,
}

#[derive(Debug, Clone)]
pub struct HookCall {
    pub kind: HookKind,
    pub line: usize,
    pub vi_calls: Vec<String>,
}
