use std::path::Path;

use crate::models::{
    Category, HookKind, ModuleGraph, ParsedModule, Severity, TestBlock, TestRuntime, Violation,
};
use crate::rules::Rule;

pub struct NoAssertionRule;

impl Rule for NoAssertionRule {
    fn id(&self) -> &'static str {
        "VITEST-MNT-001"
    }
    fn name(&self) -> &'static str {
        "NoAssertionRule"
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn applies_to_runtime(&self, _runtime: TestRuntime) -> bool {
        true
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        module
            .test_blocks
            .iter()
            .filter(|tb| !tb.has_assertions)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: "Test has no assertions \u{2014} it will pass even if the code is broken"
                    .to_string(),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Add at least one expect() assertion to verify behavior".to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

/// Flags tests with more than 5 `expect()` calls, suggesting they should
/// be split into focused, single-behavior tests.
pub struct MultipleExpectRule;

impl Rule for MultipleExpectRule {
    fn id(&self) -> &'static str {
        "VITEST-MNT-002"
    }
    fn name(&self) -> &'static str {
        "MultipleExpectRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        module
            .test_blocks
            .iter()
            .filter(|tb| tb.assertion_count > 5)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: format!(
                    "Test has {} assertions \u{2014} consider splitting into focused tests",
                    tb.assertion_count
                ),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Each test should verify one behavior. Split multiple assertions into separate tests"
                        .to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

/// Flags tests that contain `if`/`switch` statements — tests should be
/// deterministic without conditional branching.
pub struct ConditionalLogicRule;

impl Rule for ConditionalLogicRule {
    fn id(&self) -> &'static str {
        "VITEST-MNT-003"
    }
    fn name(&self) -> &'static str {
        "ConditionalLogicRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        module
            .test_blocks
            .iter()
            .filter(|tb| tb.has_conditional_logic)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: "Test contains conditional logic (if/switch) \u{2014} tests should be deterministic"
                    .to_string(),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Replace conditional logic with separate test cases using test.each() or describe.each()"
                        .to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

/// Flags tests that use `try/catch` — prefer `expect().toThrow()` or
/// `expect().rejects` for error testing.
pub struct TryCatchRule;

impl Rule for TryCatchRule {
    fn id(&self) -> &'static str {
        "VITEST-MNT-004"
    }
    fn name(&self) -> &'static str {
        "TryCatchRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        module
            .test_blocks
            .iter()
            .filter(|tb| tb.has_try_catch)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: "Test uses try/catch \u{2014} use expect(() => fn()).toThrow() instead"
                    .to_string(),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Use expect().toThrow() or expect().rejects for error testing".to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

/// Flags skipped tests (`it.skip`, `test.todo`) that provide no value
/// and should either be fixed or removed.
pub struct EmptyTestRule;

impl Rule for EmptyTestRule {
    fn id(&self) -> &'static str {
        "VITEST-MNT-005"
    }
    fn name(&self) -> &'static str {
        "EmptyTestRule"
    }
    fn severity(&self) -> Severity {
        Severity::Info
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn applies_to_runtime(&self, _runtime: TestRuntime) -> bool {
        true
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        module
            .test_blocks
            .iter()
            .filter(|tb| tb.is_skipped)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: "Skipped test detected \u{2014} skipped tests provide no value"
                    .to_string(),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Either fix the test or remove it. Avoid leaving skipped tests in the codebase"
                        .to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

/// Flags tests inside deeply nested `describe` blocks (deeper than 3 levels),
/// which harms readability and should be flattened.
pub struct NestedDescribeRule;

impl Rule for NestedDescribeRule {
    fn id(&self) -> &'static str {
        "VITEST-STR-001"
    }
    fn name(&self) -> &'static str {
        "NestedDescribeRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Structure
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        module
            .test_blocks
            .iter()
            .filter(|tb| tb.is_nested)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: "Deeply nested describe blocks \u{2014} consider flattening test structure"
                    .to_string(),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Keep describe nesting to 2 levels. Use descriptive test names instead of deep nesting"
                        .to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

/// Flags tests that use `return` statements — tests should use assertions
/// to verify behavior, not return values.
pub struct ReturnInTestRule;

impl Rule for ReturnInTestRule {
    fn id(&self) -> &'static str {
        "VITEST-STR-002"
    }
    fn name(&self) -> &'static str {
        "ReturnInTestRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Structure
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        module
            .test_blocks
            .iter()
            .filter(|tb| tb.has_return_statement)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: "Return statement in test \u{2014} tests should use assertions, not return values"
                    .to_string(),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Replace return with explicit expect() assertions".to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

/// Flags tests with unawaited `.resolves` or `.rejects` assertions that
/// will fail silently without `await`.
pub struct MissingAwaitAssertionRule;

impl Rule for MissingAwaitAssertionRule {
    fn id(&self) -> &'static str {
        "VITEST-MNT-006"
    }
    fn name(&self) -> &'static str {
        "MissingAwaitAssertionRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn applies_to_runtime(&self, _runtime: TestRuntime) -> bool {
        true
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        module
            .test_blocks
            .iter()
            .filter(|tb| tb.unawaited_async_assertions > 0)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: format!(
                    "Test has {} unawaited async assertions — these will fail silently",
                    tb.unawaited_async_assertions
                ),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Add await before expect() for .resolves or .rejects assertions".to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

/// Flags focused tests (`it.only`, `test.only`, `describe.only`) that
/// skip all other tests in the file — a common CI failure.
pub struct FocusedTestRule;

impl Rule for FocusedTestRule {
    fn id(&self) -> &'static str {
        "VITEST-MNT-007"
    }
    fn name(&self) -> &'static str {
        "FocusedTestRule"
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn applies_to_runtime(&self, _runtime: TestRuntime) -> bool {
        true
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        let mut out = Vec::new();

        for tb in &module.test_blocks {
            if tb.is_only {
                out.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: format!(
                        "Focused test '{}' uses .only — all other tests in this file will be skipped",
                        tb.name
                    ),
                    file_path: tb.file_path.clone(),
                    line: tb.line,
                    col: None,
                    suggestion: Some(
                        "Remove .only before committing. Focused tests mask failures in CI".to_string(),
                    ),
                    test_name: Some(tb.name.clone()),
                });
            }
        }

        for db in &module.describe_blocks {
            if db.is_only {
                out.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: format!(
                        "Focused describe '{}' uses .only — all other tests in this file will be skipped",
                        db.name
                    ),
                    file_path: db.file_path.clone(),
                    line: db.line,
                    col: None,
                    suggestion: Some(
                        "Remove .only before committing. Focused tests mask failures in CI".to_string(),
                    ),
                    test_name: None,
                });
            }
        }

        out
    }
}

/// Flags files using `vi.mock()` without `afterEach` or `beforeEach` cleanup,
/// allowing mocks to leak between tests.
pub struct MissingMockCleanupRule;

const MOCK_CLEANUP_CALLS: &[&str] = &["vi.restoreAllMocks", "vi.clearAllMocks", "vi.resetAllMocks"];

impl Rule for MissingMockCleanupRule {
    fn id(&self) -> &'static str {
        "VITEST-MNT-008"
    }
    fn name(&self) -> &'static str {
        "MissingMockCleanupRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        let has_vi_mocks = !module.vi_mocks.is_empty();
        let has_global_stubs = !module.global_stubs.is_empty();

        if !has_vi_mocks && !has_global_stubs {
            return vec![];
        }

        let has_cleanup = module.hook_calls.iter().any(|h| {
            (h.kind == HookKind::AfterEach || h.kind == HookKind::BeforeEach)
                && h.vi_calls
                    .iter()
                    .any(|c| MOCK_CLEANUP_CALLS.iter().any(|mc| c == mc))
        });

        let has_unstub = module.hook_calls.iter().any(|h| {
            (h.kind == HookKind::AfterEach || h.kind == HookKind::BeforeEach)
                && h.vi_calls.iter().any(|c| c == "vi.unstubAllGlobals")
        });

        let mut violations = Vec::new();

        if has_vi_mocks && !has_cleanup {
            let first_mock = module.vi_mocks.iter().min_by_key(|m| m.line);
            if let Some(mock) = first_mock {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: format!(
                        "vi.mock('{}') used without afterEach cleanup — mocks may leak between tests",
                        mock.source
                    ),
                    file_path: module.file_path.clone(),
                    line: mock.line,
                    col: None,
                    suggestion: Some(
                        "Add afterEach(() => { vi.restoreAllMocks() }) or beforeEach(() => { vi.clearAllMocks() }) to clean up mocks between tests"
                            .to_string(),
                    ),
                    test_name: None,
                });
            }
        }

        if has_global_stubs && !has_cleanup && !has_unstub {
            let first_stub = module.global_stubs.iter().min_by_key(|s| s.line);
            if let Some(stub) = first_stub {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: format!(
                        "global.{} stub without cleanup — may leak between tests",
                        stub.target
                    ),
                    file_path: module.file_path.clone(),
                    line: stub.line,
                    col: None,
                    suggestion: Some(
                        "Add afterEach(() => { vi.unstubAllGlobals() }) or restore the original value manually".to_string(),
                    ),
                    test_name: None,
                });
            }
        }

        violations
    }
}

/// Flags tests where all assertions are weak — `toBeDefined()`,
/// `toBeTruthy()`, `not.toThrow()`, etc. — that verify existence or
/// truthiness rather than actual values.
pub struct WeakAssertionRule;

impl Rule for WeakAssertionRule {
    fn id(&self) -> &'static str {
        "VITEST-MNT-009"
    }
    fn name(&self) -> &'static str {
        "WeakAssertionRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        module
            .test_blocks
            .iter()
            .filter(|tb| tb.assertion_count > 0 && tb.weak_assertion_count == tb.assertion_count)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: format!(
                    "All {} assertion(s) in this test are weak — they verify existence or truthiness, not actual behavior",
                    tb.assertion_count
                ),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Use specific assertions like toBe(), toEqual(), or toHaveLength() instead of toBeDefined()/toBeTruthy()/not.toThrow()"
                        .to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

/// Flags test files that are tightly coupled to a single module's implementation —
/// the test imports one production module, test count ≈ export count, and >80% of
/// test names match export names. Such tests break on any refactor and provide
/// low-value coverage.
pub struct ImplementationCoupledRule;

impl Rule for ImplementationCoupledRule {
    fn id(&self) -> &'static str {
        "VITEST-MNT-010"
    }
    fn name(&self) -> &'static str {
        "ImplementationCoupledRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn applies_to_runtime(&self, _runtime: TestRuntime) -> bool {
        true
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        graph: &ModuleGraph,
    ) -> Vec<Violation> {
        self.check_export_coupling(module, _ctx, graph)
            .into_iter()
            .collect()
    }
}

impl ImplementationCoupledRule {
    fn check_export_coupling(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        graph: &ModuleGraph,
    ) -> Option<Violation> {
        let prod_imports = Self::filter_prod_imports(module);

        if prod_imports.len() != 1 {
            return None;
        }

        let source_module_path = prod_imports[0];
        let source_module = Self::resolve_source_module(module, source_module_path, _ctx, graph)?;

        let export_count = source_module.exports.len();
        let test_count = module.test_blocks.len();

        if export_count == 0 || test_count == 0 {
            return None;
        }

        let ratio = test_count as f64 / export_count as f64;
        if !(0.8..=1.2).contains(&ratio) {
            return None;
        }

        let export_names: Vec<String> = source_module
            .exports
            .iter()
            .map(|e| e.name.to_lowercase())
            .collect();

        let match_ratio = Self::compute_name_match_ratio(&module.test_blocks, &export_names);
        if match_ratio < 0.8 {
            return None;
        }

        Some(Violation {
            rule_id: self.id().to_string(),
            rule_name: self.name().to_string(),
            severity: self.severity(),
            category: self.category(),
            message: format!(
                "Test file is tightly coupled to '{}' — {} tests for {} exports, {}% name match",
                source_module_path,
                test_count,
                export_count,
                (match_ratio * 100.0) as u32
            ),
            file_path: module.file_path.clone(),
            line: 1,
            col: None,
            suggestion: Some(
                "Test behavior, not implementation details. Refactor tests to verify public API outcomes rather than mirroring export structure"
                    .to_string(),
            ),
            test_name: None,
        })
    }

    /// Filters out test-framework / node_modules imports, keeping only production imports.
    fn filter_prod_imports(module: &ParsedModule) -> Vec<&str> {
        module
            .imports_parsed
            .iter()
            .filter(|imp| {
                !imp.source.starts_with("vitest")
                    && !imp.source.starts_with("@testing-library")
                    && !imp.source.starts_with("jest")
                    && !imp.source.starts_with("@jest")
                    && !imp.source.contains("node_modules")
            })
            .map(|imp| imp.source.as_str())
            .collect()
    }

    /// Resolves an import path to a parsed module, trying relative paths with extensions
    /// and index files when the direct path doesn't match.
    fn resolve_source_module<'a>(
        module: &ParsedModule,
        source_path: &str,
        ctx: &crate::rules::LintContext<'_>,
        graph: &'a ModuleGraph,
    ) -> Option<&'a ParsedModule> {
        let resolved = ctx.config.resolve_module_path(source_path);
        if let Some(m) = graph.get_module(Path::new(&resolved)) {
            return Some(m);
        }
        if !resolved.starts_with('.') && !resolved.starts_with('/') {
            return None;
        }
        let parent = module.file_path.parent()?;
        let base = parent.join(&resolved);
        let exts = ["ts", "tsx", "js", "jsx"];
        try_resolve_extension(&base, &exts, graph)
            .or_else(|| try_resolve_index(&base, &exts, graph))
    }

    /// Computes the fraction of test blocks whose names contain at least one export name.
    fn compute_name_match_ratio(test_blocks: &[TestBlock], export_names: &[String]) -> f64 {
        let matching = test_blocks
            .iter()
            .filter(|tb| {
                let test_name_lower = tb.name.to_lowercase();
                export_names
                    .iter()
                    .any(|en| contains_word(&test_name_lower, en))
            })
            .count();
        matching as f64 / test_blocks.len() as f64
    }
}

/// Try to resolve a module by appending each extension to the base path.
fn try_resolve_extension<'a>(
    base: &Path,
    exts: &[&str],
    graph: &'a ModuleGraph,
) -> Option<&'a ParsedModule> {
    for ext in exts {
        let candidate = base.with_extension(ext);
        if let Some(m) = graph.get_module(&candidate) {
            return Some(m);
        }
    }
    None
}

/// Try to resolve a module by looking for index files with each extension.
fn try_resolve_index<'a>(
    base: &Path,
    exts: &[&str],
    graph: &'a ModuleGraph,
) -> Option<&'a ParsedModule> {
    for ext in exts {
        let candidate = base.join(format!("index.{}", ext));
        if let Some(m) = graph.get_module(&candidate) {
            return Some(m);
        }
    }
    None
}

pub struct TestIdNegativePresenceRule;

impl Rule for TestIdNegativePresenceRule {
    fn id(&self) -> &'static str {
        "VITEST-MNT-011"
    }
    fn name(&self) -> &'static str {
        "TestIdNegativePresenceRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn applies_to_runtime(&self, _runtime: TestRuntime) -> bool {
        true
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        let pw_testid_count = module
            .playwright
            .as_ref()
            .map(|pw| {
                pw.locator_chains
                    .iter()
                    .filter(|c| c.root == "getByTestId")
                    .count()
            })
            .unwrap_or(0);
        if pw_testid_count == 0 {
            return vec![];
        }
        let has_negative = module
            .playwright
            .as_ref()
            .map(|pw| pw.locator_chains.iter().any(|c| c.root == "queryByTestId"))
            .unwrap_or(false);
        if has_negative {
            return vec![];
        }
        let first_line = module
            .playwright
            .as_ref()
            .and_then(|pw| {
                pw.locator_chains
                    .iter()
                    .find(|c| c.root == "getByTestId")
                    .map(|c| c.line)
            })
            .unwrap_or(1);
        vec![Violation {
            rule_id: self.id().to_string(),
            rule_name: self.name().to_string(),
            severity: self.severity(),
            category: self.category(),
            message: "Test uses getByTestId but has no negative-presence assertion — missing coverage for element absence".to_string(),
            file_path: module.file_path.clone(),
            line: first_line,
            col: None,
            suggestion: Some("Add expect(queryByTestId('x')).toBeNull() or expect(getByTestId('x')).not.toBeVisible()".to_string()),
            test_name: None,
        }]
    }
}

/// Check if `word` appears as a whole word in `text`.
/// Word boundaries are non-alphanumeric characters or start/end of string.
fn contains_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let mut start = 0;
    while let Some(pos) = text[start..].find(word) {
        let abs_pos = start + pos;
        let before_ok = abs_pos == 0 || !text.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
        let after_pos = abs_pos + word.len();
        let after_ok =
            after_pos >= text.len() || !text.as_bytes()[after_pos].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        start = abs_pos + 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ImportEntry;
    use std::path::PathBuf;

    fn empty_module() -> ParsedModule {
        ParsedModule {
            file_path: PathBuf::from("test.ts"),
            test_blocks: vec![],
            describe_blocks: vec![],
            imports: vec![],
            imports_parsed: vec![],
            hook_calls: vec![],
            vi_mocks: vec![],
            exports: vec![],
            has_fake_timers: false,
            expects_outside_tests: vec![],
            imports_node_test: false,
            snapshot_sizes: vec![],
            runtime: crate::models::TestRuntime::Unknown,
            playwright: None,
            global_stubs: vec![],
            ..ParsedModule::default()
        }
    }

    fn import_entry(source: &str) -> ImportEntry {
        ImportEntry {
            source: source.to_string(),
            named: vec![],
            default: None,
            namespace: None,
            line: 1,
        }
    }

    #[test]
    fn implementation_coupled_no_prod_imports() {
        let mut module = empty_module();
        module.imports_parsed = vec![
            import_entry("vitest"),
            import_entry("@testing-library/react"),
        ];
        let rule = ImplementationCoupledRule;
        let violations = rule.check(
            &module,
            &crate::rules::LintContext::default(),
            &ModuleGraph::new(&[], &[]),
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn implementation_coupled_multiple_prod_imports() {
        let mut module = empty_module();
        module.imports_parsed = vec![
            import_entry("vitest"),
            import_entry("lodash"),
            import_entry("axios"),
        ];
        let rule = ImplementationCoupledRule;
        let violations = rule.check(
            &module,
            &crate::rules::LintContext::default(),
            &ModuleGraph::new(&[], &[]),
        );
        assert!(violations.is_empty());
    }
}
