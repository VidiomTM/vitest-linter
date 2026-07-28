use std::collections::HashSet;

use crate::models::{Category, ModuleGraph, ParsedModule, Severity, TestRuntime, Violation};
use crate::rules::Rule;

// ---------------------------------------------------------------------------
// VITEST-NO-001: NoStandaloneExpectRule
// ---------------------------------------------------------------------------

pub struct NoStandaloneExpectRule;

impl Rule for NoStandaloneExpectRule {
    fn id(&self) -> &'static str {
        "VITEST-NO-001"
    }
    fn name(&self) -> &'static str {
        "NoStandaloneExpectRule"
    }
    fn severity(&self) -> Severity {
        Severity::Error
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
            .expects_outside_tests
            .iter()
            .map(|e| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: "expect() called outside of a test or it() block".to_string(),
                file_path: module.file_path.clone(),
                line: e.line,
                col: None,
                suggestion: Some("Move expect() inside a test() or it() callback".to_string()),
                test_name: None,
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// VITEST-NO-002: NoIdenticalTitleRule
// ---------------------------------------------------------------------------

pub struct NoIdenticalTitleRule;

impl Rule for NoIdenticalTitleRule {
    fn id(&self) -> &'static str {
        "VITEST-NO-002"
    }
    fn name(&self) -> &'static str {
        "NoIdenticalTitleRule"
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn category(&self) -> Category {
        Category::Structure
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
        let mut violations = Vec::new();
        let mut seen_titles: HashSet<&str> = HashSet::new();

        for tb in &module.test_blocks {
            if !seen_titles.insert(&tb.name) {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: format!("Duplicate test title: '{}'", tb.name),
                    file_path: tb.file_path.clone(),
                    line: tb.line,
                    col: None,
                    suggestion: Some("Give each test a unique title".to_string()),
                    test_name: Some(tb.name.clone()),
                });
            }
        }

        let mut seen_describe: HashSet<&str> = HashSet::new();
        for db in &module.describe_blocks {
            if !db.name.is_empty() && !seen_describe.insert(&db.name) {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: format!("Duplicate describe title: '{}'", db.name),
                    file_path: db.file_path.clone(),
                    line: db.line,
                    col: None,
                    suggestion: Some("Give each describe block a unique title".to_string()),
                    test_name: None,
                });
            }
        }

        violations
    }
}

// ---------------------------------------------------------------------------
// VITEST-NO-003: NoCommentedOutTestsRule
// ---------------------------------------------------------------------------

pub struct NoCommentedOutTestsRule;

/// Collapse the whitespace immediately following a line-comment marker so that
/// `//test(`, `//   test(`, and `// test(` are all matched by the `// test(`
/// pattern set. Returns the input unchanged when no `//` marker is present.
fn normalize_comment_spacing(line: &str) -> String {
    if let Some(rest) = line.strip_prefix("//") {
        let trimmed_rest = rest.trim_start();
        format!("// {trimmed_rest}")
    } else {
        line.to_string()
    }
}

const COMMENTED_TEST_PATTERNS: &[&str] = &[
    "// test(",
    "// test.skip(",
    "// test.only(",
    "// it(",
    "// it.skip(",
    "// it.only(",
    "// describe(",
    "// describe.skip(",
    "// describe.only(",
];

impl Rule for NoCommentedOutTestsRule {
    fn id(&self) -> &'static str {
        "VITEST-NO-003"
    }
    fn name(&self) -> &'static str {
        "NoCommentedOutTestsRule"
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
        let source = &module.source;

        let mut violations = Vec::new();
        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            // Normalize the comment marker so `//test(` and `//   test(`
            // both match the `// test(` patterns. Only the run of whitespace
            // immediately following `//` is collapsed to a single space.
            let normalized = normalize_comment_spacing(trimmed);
            for pattern in COMMENTED_TEST_PATTERNS {
                if normalized.contains(pattern) {
                    violations.push(Violation {
                        rule_id: self.id().to_string(),
                        rule_name: self.name().to_string(),
                        severity: self.severity(),
                        category: self.category(),
                        message: "Remove commented-out test code".to_string(),
                        file_path: module.file_path.clone(),
                        line: line_idx + 1,
                        col: None,
                        suggestion: Some(
                            "Either restore or delete commented-out tests".to_string(),
                        ),
                        test_name: None,
                    });
                    break; // one violation per line
                }
            }
        }

        violations
    }
}

// ---------------------------------------------------------------------------
// VITEST-NO-005: NoTestPrefixesRule
// ---------------------------------------------------------------------------

pub struct NoTestPrefixesRule;

impl Rule for NoTestPrefixesRule {
    fn id(&self) -> &'static str {
        "VITEST-NO-005"
    }
    fn name(&self) -> &'static str {
        "NoTestPrefixesRule"
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
            .filter(|tb| tb.uses_fit_or_xit)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: format!(
                    "Test '{}' uses fit()/xit() — use test.skip()/test.only() instead",
                    tb.name
                ),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Replace fit() with test.only() and xit() with test.skip()".to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// VITEST-NO-006: NoDuplicateHooksRule
// ---------------------------------------------------------------------------

pub struct NoDuplicateHooksRule;

impl Rule for NoDuplicateHooksRule {
    fn id(&self) -> &'static str {
        "VITEST-NO-006"
    }
    fn name(&self) -> &'static str {
        "NoDuplicateHooksRule"
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
        use crate::models::HookKind;

        let mut violations = Vec::new();
        let mut seen_before_each = false;
        let mut seen_after_each = false;
        let mut seen_before_all = false;
        let mut seen_after_all = false;

        for hook in &module.hook_calls {
            match hook.kind {
                HookKind::BeforeEach => {
                    if seen_before_each {
                        violations.push(Violation {
                            rule_id: self.id().to_string(),
                            rule_name: self.name().to_string(),
                            severity: self.severity(),
                            category: self.category(),
                            message: "Duplicate beforeEach hook".to_string(),
                            file_path: module.file_path.clone(),
                            line: hook.line,
                            col: None,
                            suggestion: Some(
                                "Consolidate duplicate beforeEach hooks into one".to_string(),
                            ),
                            test_name: None,
                        });
                    }
                    seen_before_each = true;
                }
                HookKind::AfterEach => {
                    if seen_after_each {
                        violations.push(Violation {
                            rule_id: self.id().to_string(),
                            rule_name: self.name().to_string(),
                            severity: self.severity(),
                            category: self.category(),
                            message: "Duplicate afterEach hook".to_string(),
                            file_path: module.file_path.clone(),
                            line: hook.line,
                            col: None,
                            suggestion: Some(
                                "Consolidate duplicate afterEach hooks into one".to_string(),
                            ),
                            test_name: None,
                        });
                    }
                    seen_after_each = true;
                }
                HookKind::BeforeAll => {
                    if seen_before_all {
                        violations.push(Violation {
                            rule_id: self.id().to_string(),
                            rule_name: self.name().to_string(),
                            severity: self.severity(),
                            category: self.category(),
                            message: "Duplicate beforeAll hook".to_string(),
                            file_path: module.file_path.clone(),
                            line: hook.line,
                            col: None,
                            suggestion: Some(
                                "Consolidate duplicate beforeAll hooks into one".to_string(),
                            ),
                            test_name: None,
                        });
                    }
                    seen_before_all = true;
                }
                HookKind::AfterAll => {
                    if seen_after_all {
                        violations.push(Violation {
                            rule_id: self.id().to_string(),
                            rule_name: self.name().to_string(),
                            severity: self.severity(),
                            category: self.category(),
                            message: "Duplicate afterAll hook".to_string(),
                            file_path: module.file_path.clone(),
                            line: hook.line,
                            col: None,
                            suggestion: Some(
                                "Consolidate duplicate afterAll hooks into one".to_string(),
                            ),
                            test_name: None,
                        });
                    }
                    seen_after_all = true;
                }
            }
        }

        violations
    }
}

// ---------------------------------------------------------------------------
// VITEST-NO-007: NoImportNodeTestRule
// ---------------------------------------------------------------------------

pub struct NoImportNodeTestRule;

impl Rule for NoImportNodeTestRule {
    fn id(&self) -> &'static str {
        "VITEST-NO-007"
    }
    fn name(&self) -> &'static str {
        "NoImportNodeTestRule"
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn category(&self) -> Category {
        Category::Dependencies
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        if module.imports_node_test {
            vec![Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: "Do not import from 'node:test' — use vitest APIs instead".to_string(),
                file_path: module.file_path.clone(),
                line: module
                    .imports_parsed
                    .iter()
                    .find(|i| i.source == "node:test")
                    .map_or(1, |i| i.line),
                col: None,
                suggestion: Some("Replace 'node:test' imports with vitest equivalents".to_string()),
                test_name: None,
            }]
        } else {
            vec![]
        }
    }
}

// ---------------------------------------------------------------------------
// VITEST-NO-008: NoInterpolationInSnapshotsRule
// ---------------------------------------------------------------------------

pub struct NoInterpolationInSnapshotsRule;

impl Rule for NoInterpolationInSnapshotsRule {
    fn id(&self) -> &'static str {
        "VITEST-NO-008"
    }
    fn name(&self) -> &'static str {
        "NoInterpolationInSnapshotsRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Validation
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        let source = &module.source;

        let mut violations = Vec::new();

        // Scan for template literals inside snapshot matcher calls.
        // Pattern: .toMatchInlineSnapshot(`...${...}...`) or .toMatchSnapshot(`...${...}...`)
        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if (trimmed.contains("toMatchInlineSnapshot(`")
                || trimmed.contains("toMatchSnapshot(`"))
                && trimmed.contains("${")
            {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: "Avoid template literal interpolation in snapshots".to_string(),
                    file_path: module.file_path.clone(),
                    line: line_idx + 1,
                    col: None,
                    suggestion: Some(
                        "Use a static string in snapshots to ensure deterministic output"
                            .to_string(),
                    ),
                    test_name: None,
                });
            }
        }

        violations
    }
}

// ---------------------------------------------------------------------------
// VITEST-NO-009: NoLargeSnapshotsRule
// ---------------------------------------------------------------------------

pub struct NoLargeSnapshotsRule;

const DEFAULT_MAX_SNAPSHOT_LINES: usize = 50;

impl Rule for NoLargeSnapshotsRule {
    fn id(&self) -> &'static str {
        "VITEST-NO-009"
    }
    fn name(&self) -> &'static str {
        "NoLargeSnapshotsRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Validation
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        module
            .snapshot_sizes
            .iter()
            .filter(|s| s.size > DEFAULT_MAX_SNAPSHOT_LINES)
            .map(|s| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: format!(
                    "Snapshot is {} lines (max {})",
                    s.size, DEFAULT_MAX_SNAPSHOT_LINES
                ),
                file_path: module.file_path.clone(),
                line: s.line,
                col: None,
                suggestion: Some(
                    "Break large snapshots into smaller ones or use inline assertions".to_string(),
                ),
                test_name: None,
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// VITEST-NO-013: NoDoneCallbackRule
// ---------------------------------------------------------------------------

pub struct NoDoneCallbackRule;

impl Rule for NoDoneCallbackRule {
    fn id(&self) -> &'static str {
        "VITEST-NO-013"
    }
    fn name(&self) -> &'static str {
        "NoDoneCallbackRule"
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
            .filter(|tb| tb.has_done_callback)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: format!("Test '{}' uses done callback — prefer async/await", tb.name),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Convert to async function and use await instead of done callback".to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// VITEST-NO-014: NoConditionalExpectRule
// ---------------------------------------------------------------------------

pub struct NoConditionalExpectRule;

impl Rule for NoConditionalExpectRule {
    fn id(&self) -> &'static str {
        "VITEST-NO-014"
    }
    fn name(&self) -> &'static str {
        "NoConditionalExpectRule"
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
            .filter(|tb| tb.has_conditional_expect)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: format!(
                    "Test '{}' has expect() inside conditional — assertions should be unconditional",
                    tb.name
                ),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Move expect() outside of if/switch blocks or split into separate tests"
                        .to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::parser::TsParser;
    use std::path::Path;

    #[test]
    fn commented_test_rule_reads_retained_source_without_disk() {
        // Regression: source-reading rules used to re-read `module.file_path`
        // from disk and silently returned no diagnostics when the path was
        // unreadable (in-memory/unsaved inputs, stdin). They now consume the
        // retained `module.source`, so the rule fires even when the file path
        // does not exist on disk.
        let parser = TsParser::new().unwrap();
        let module = parser
            .parse_source(
                "// test('commented out', () => {});\n",
                Path::new("/nonexistent/does-not-exist.test.ts"),
            )
            .unwrap();
        let config = Config::default();
        let ctx = crate::rules::LintContext {
            config: &config,
            all_modules: &[],
        };
        let v = NoCommentedOutTestsRule.check(&module, &ctx, &ModuleGraph::default());
        assert_eq!(
            v.len(),
            1,
            "rule must fire from retained source without disk"
        );
        assert_eq!(v[0].rule_id, "VITEST-NO-003");
    }
}
