use crate::config::Config;
use crate::models::{Category, ModuleGraph, ParsedModule, Severity, TestRuntime, Violation};

/// Context passed to each rule during evaluation, including the active
/// configuration and all modules in the current group.
pub struct LintContext<'a> {
    pub config: &'a Config,
    pub all_modules: &'a [ParsedModule],
}

impl Default for LintContext<'_> {
    fn default() -> Self {
        use std::sync::OnceLock;
        static DEFAULT_CONFIG: OnceLock<Config> = OnceLock::new();
        Self {
            config: DEFAULT_CONFIG.get_or_init(Config::default),
            all_modules: &[],
        }
    }
}

/// Trait implemented by every lint rule.
pub trait Rule {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn severity(&self) -> Severity;
    fn category(&self) -> Category;
    fn check(
        &self,
        module: &ParsedModule,
        ctx: &LintContext<'_>,
        graph: &ModuleGraph,
    ) -> Vec<Violation>;

    /// Whether this rule should fire for the given test runtime.
    /// Default: only Vitest/Unknown (not Playwright).
    /// Override to `true` for rules that apply to both runtimes.
    fn applies_to_runtime(&self, runtime: TestRuntime) -> bool {
        runtime != TestRuntime::Playwright
    }
}

pub mod consistency;
pub mod dependencies;
pub mod flakiness;
pub mod maintenance;
pub mod no_category;
pub mod playwright;
pub mod prefer;
pub mod require;
pub mod selector_classifier;
pub mod validation;

/// The 8 community-hit rules active by default for v1.0.
///
/// These catch the most real-world pain with high signal and low false-positive
/// rate. Use `all_rules()` for the full set (requires `--unstable-rules`).
#[must_use]
pub fn v1_0_rules() -> Vec<Box<dyn Rule>> {
    vec![
        // FocusedTestRule — it.only / describe.only left in committed code
        Box::new(maintenance::FocusedTestRule),
        // EmptyTestRule — it.skip / test.todo left in source
        Box::new(maintenance::EmptyTestRule),
        // NoCommentedOutTestsRule — commented-out test bodies
        Box::new(no_category::NoCommentedOutTestsRule),
        // MissingAwaitAssertionRule — async test with no await (silent pass)
        Box::new(maintenance::MissingAwaitAssertionRule),
        // TimeoutRule — real setTimeout in tests (flaky-time)
        Box::new(flakiness::TimeoutRule),
        // TryCatchRule — try block with no expect.fail in catch
        Box::new(maintenance::TryCatchRule),
        // ConditionalLogicRule — conditional logic / state leakage in describe
        Box::new(maintenance::ConditionalLogicRule),
        // PreferHooksOnTopRule — hooks declared after first test
        Box::new(prefer::PreferHooksOnTopRule),
    ]
}

#[must_use]
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(flakiness::TimeoutRule),
        Box::new(flakiness::DateMockRule),
        Box::new(flakiness::NetworkImportRule),
        Box::new(flakiness::FakeTimersCleanupRule),
        Box::new(flakiness::NonDeterministicRule),
        Box::new(maintenance::NoAssertionRule),
        Box::new(maintenance::MultipleExpectRule),
        Box::new(maintenance::ConditionalLogicRule),
        Box::new(maintenance::TryCatchRule),
        Box::new(maintenance::EmptyTestRule),
        Box::new(maintenance::NestedDescribeRule),
        Box::new(maintenance::ReturnInTestRule),
        Box::new(maintenance::MissingAwaitAssertionRule),
        Box::new(maintenance::FocusedTestRule),
        Box::new(maintenance::MissingMockCleanupRule),
        Box::new(maintenance::WeakAssertionRule),
        Box::new(maintenance::ImplementationCoupledRule),
        Box::new(maintenance::TestIdNegativePresenceRule),
        Box::new(dependencies::BannedModuleMockRule),
        Box::new(dependencies::ProductionSingletonImportRule),
        Box::new(dependencies::ResetEscapeHatchRule),
        Box::new(dependencies::MockExportValidationRule),
        Box::new(validation::ValidExpectRule),
        Box::new(validation::ValidExpectInPromiseRule),
        Box::new(validation::ValidDescribeCallbackRule),
        Box::new(validation::ValidTitleRule),
        Box::new(validation::NoUnneededAsyncExpectFunctionRule),
        // E12: No-rules
        Box::new(no_category::NoStandaloneExpectRule),
        Box::new(no_category::NoIdenticalTitleRule),
        Box::new(no_category::NoCommentedOutTestsRule),
        Box::new(no_category::NoTestPrefixesRule),
        Box::new(no_category::NoDuplicateHooksRule),
        Box::new(no_category::NoImportNodeTestRule),
        Box::new(no_category::NoInterpolationInSnapshotsRule),
        Box::new(no_category::NoLargeSnapshotsRule),
        Box::new(no_category::NoDoneCallbackRule),
        Box::new(no_category::NoConditionalExpectRule),
        // E13: Prefer-rules
        Box::new(prefer::PreferToBeRule),
        Box::new(prefer::PreferToContainRule),
        Box::new(prefer::PreferToHaveLengthRule),
        Box::new(prefer::PreferSpyOnRule),
        Box::new(prefer::PreferCalledOnceRule),
        Box::new(prefer::PreferHooksOnTopRule),
        Box::new(prefer::PreferHooksInOrderRule),
        Box::new(prefer::PreferTodoRule),
        Box::new(prefer::PreferMockPromiseShorthandRule),
        Box::new(prefer::PreferExpectResolvesRule),
        // E14: Require-rules
        Box::new(require::RequireHookRule),
        Box::new(require::RequireTopLevelDescribeRule),
        Box::new(require::RequireToThrowMessageRule),
        // E15: Consistency-rules
        Box::new(consistency::ConsistentTestItRule),
        Box::new(consistency::ConsistentVitestViRule),
        Box::new(consistency::HoistedApisOnTopRule),
        // Playwright rules
        Box::new(playwright::PwWaitForTimeoutRule),
        Box::new(playwright::PwCssIdSelectorRule),
        Box::new(playwright::PwXPathSelectorRule),
        Box::new(playwright::PwLocatorNthRule),
        Box::new(playwright::PwPageDollarRule),
        Box::new(playwright::PwEvaluateInnerTextRule),
        Box::new(playwright::PwArbitrarySleepRule),
        Box::new(playwright::PwHardCssClassChainRule),
        Box::new(playwright::PwDuplicateSpecFileRule),
        Box::new(playwright::PwTextAssertionOverRoleRule),
        Box::new(playwright::PwTestIdOverSemanticRoleRule),
        Box::new(playwright::PwMissingWebFirstAssertionRule),
        Box::new(playwright::PwMissingAxeScanRule),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_rules_count() {
        let rules = all_rules();
        assert_eq!(rules.len(), 66);
    }

    #[test]
    fn v1_0_rules_count() {
        let rules = v1_0_rules();
        assert_eq!(rules.len(), 8);
    }

    #[test]
    fn v1_0_rules_are_subset_of_all() {
        let all = all_rules();
        let v1 = v1_0_rules();
        for rule in &v1 {
            assert!(
                all.iter().any(|r| r.id() == rule.id()),
                "v1.0 rule {} not found in all_rules()",
                rule.id()
            );
        }
    }

    #[test]
    fn v1_0_rule_ids() {
        let rules = v1_0_rules();
        let ids: Vec<&str> = rules.iter().map(|r| r.id()).collect();
        let expected = [
            "VITEST-MNT-007",  // FocusedTestRule
            "VITEST-MNT-005",  // EmptyTestRule
            "VITEST-NO-003",   // NoCommentedOutTestsRule
            "VITEST-MNT-006",  // MissingAwaitAssertionRule
            "VITEST-FLK-001",  // TimeoutRule
            "VITEST-MNT-004",  // TryCatchRule
            "VITEST-MNT-003",  // ConditionalLogicRule
            "VITEST-PREF-009", // PreferHooksOnTopRule
        ];
        for id in &expected {
            assert!(ids.contains(id), "Missing v1.0 rule: {}", id);
        }
        assert_eq!(ids.len(), expected.len());
    }

    #[test]
    fn all_rule_ids_present() {
        let rules = all_rules();
        let expected = [
            "VITEST-FLK-001",
            "VITEST-FLK-002",
            "VITEST-FLK-003",
            "VITEST-FLK-004",
            "VITEST-FLK-005",
            "VITEST-MNT-001",
            "VITEST-MNT-002",
            "VITEST-MNT-003",
            "VITEST-MNT-004",
            "VITEST-MNT-005",
            "VITEST-STR-001",
            "VITEST-STR-002",
            "VITEST-MNT-006",
            "VITEST-MNT-007",
            "VITEST-MNT-008",
            "VITEST-MNT-009",
            "VITEST-MNT-010",
            "VITEST-DEP-001",
            "VITEST-DEP-002",
            "VITEST-DEP-003",
            "VITEST-DEP-004",
            "VITEST-VAL-001",
            "VITEST-VAL-002",
            "VITEST-VAL-003",
            "VITEST-VAL-004",
            "VITEST-VAL-005",
            "VITEST-NO-001",
            "VITEST-NO-002",
            "VITEST-NO-003",
            "VITEST-NO-005",
            "VITEST-NO-006",
            "VITEST-NO-007",
            "VITEST-NO-008",
            "VITEST-NO-009",
            "VITEST-NO-013",
            "VITEST-NO-014",
            "VITEST-PREF-001",
            "VITEST-PREF-002",
            "VITEST-PREF-003",
            "VITEST-PREF-005",
            "VITEST-PREF-007",
            "VITEST-PREF-009",
            "VITEST-PREF-010",
            "VITEST-PREF-012",
            "VITEST-PREF-013",
            "VITEST-PREF-014",
            "VITEST-REQ-001",
            "VITEST-REQ-002",
            "VITEST-REQ-003",
            "VITEST-CON-001",
            "VITEST-CON-003",
            "VITEST-CON-004",
        ];
        let ids: Vec<&str> = rules.iter().map(|r| r.id()).collect();
        for id in &expected {
            assert!(ids.contains(id), "Missing rule: {}", id);
        }
    }

    #[test]
    fn all_rules_unique_ids() {
        let rules = all_rules();
        let ids: Vec<&str> = rules.iter().map(|r| r.id()).collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "Duplicate rule IDs found");
    }

    /// Cross-check: the set of rule IDs implemented in Rust must exactly match
    /// the set exposed by the ESLint plugin (`eslint-plugin-vitest-linter/lib/rules.js`).
    /// This test embeds the eslint rule IDs so that any drift (a rule added on
    /// one side but not the other, or a mismatched ID like the pw-page-dollar /
    /// VITEST-PW-010 bug) is caught at test time. When adding/removing a rule,
    /// update both this list and `eslint-plugin-vitest-linter/lib/rules.js`.
    #[test]
    fn rust_rule_ids_match_eslint_plugin_map() {
        let rust_ids: std::collections::BTreeSet<&str> =
            all_rules().iter().map(|r| r.id()).collect();

        // Mirrors eslint-plugin-vitest-linter/lib/rules.js ruleId values.
        let eslint_ids: std::collections::BTreeSet<&str> = [
            "VITEST-FLK-001",
            "VITEST-FLK-002",
            "VITEST-FLK-003",
            "VITEST-FLK-004",
            "VITEST-FLK-005",
            "VITEST-MNT-001",
            "VITEST-MNT-002",
            "VITEST-MNT-003",
            "VITEST-MNT-004",
            "VITEST-MNT-005",
            "VITEST-STR-001",
            "VITEST-STR-002",
            "VITEST-MNT-006",
            "VITEST-MNT-007",
            "VITEST-MNT-008",
            "VITEST-MNT-009",
            "VITEST-MNT-010",
            "VITEST-MNT-011",
            "VITEST-DEP-001",
            "VITEST-DEP-002",
            "VITEST-DEP-003",
            "VITEST-DEP-004",
            "VITEST-VAL-001",
            "VITEST-VAL-002",
            "VITEST-VAL-003",
            "VITEST-VAL-004",
            "VITEST-VAL-005",
            "VITEST-NO-001",
            "VITEST-NO-002",
            "VITEST-NO-003",
            "VITEST-NO-005",
            "VITEST-NO-006",
            "VITEST-NO-007",
            "VITEST-NO-008",
            "VITEST-NO-009",
            "VITEST-NO-013",
            "VITEST-NO-014",
            "VITEST-PREF-001",
            "VITEST-PREF-002",
            "VITEST-PREF-003",
            "VITEST-PREF-005",
            "VITEST-PREF-007",
            "VITEST-PREF-009",
            "VITEST-PREF-010",
            "VITEST-PREF-012",
            "VITEST-PREF-013",
            "VITEST-PREF-014",
            "VITEST-REQ-001",
            "VITEST-REQ-002",
            "VITEST-REQ-003",
            "VITEST-CON-001",
            "VITEST-CON-003",
            "VITEST-CON-004",
            "VITEST-PW-001",
            "VITEST-PW-002",
            "VITEST-PW-003",
            "VITEST-PW-004",
            "VITEST-PW-005",
            "VITEST-PW-006",
            "VITEST-PW-007",
            "VITEST-PW-008",
            "VITEST-PW-009",
            "VITEST-PW-010",
            "VITEST-PW-011",
            "VITEST-PW-012",
            "VITEST-PW-100",
        ]
        .into_iter()
        .collect();

        assert_eq!(
            rust_ids, eslint_ids,
            "Rust rule IDs and ESLint plugin rule IDs must match exactly"
        );
    }

    /// Cross-check (file-scraping): read `eslint-plugin-vitest-linter/lib/rules.js`
    /// as text and assert every Rust `all_rules()` ID appears as a `ruleId:` in
    /// the JS file, and vice-versa. Unlike `rust_rule_ids_match_eslint_plugin_map`
    /// (which embeds a hardcoded list), this test reads the actual JS source so
    /// drift in *either* direction is caught without remembering to update a
    /// third copy of the list.
    #[test]
    fn rust_rule_ids_match_eslint_plugin_js_file() {
        let js_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("eslint-plugin-vitest-linter/lib/rules.js");
        let js = std::fs::read_to_string(&js_path)
            .unwrap_or_else(|e| panic!("could not read {}: {e}", js_path.display()));

        // Extract `ruleId: "VITEST-XXX-NNN"` values from the JS source.
        let mut js_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for line in js.lines() {
            if let Some(rest) = line.split("ruleId:").nth(1) {
                let rest = rest.trim_start();
                if let Some(rest) = rest.strip_prefix('"').or_else(|| rest.strip_prefix('\'')) {
                    if let Some(end) = rest.find(['"', '\'']) {
                        js_ids.insert(rest[..end].to_string());
                    }
                }
            }
        }

        let rust_ids: std::collections::BTreeSet<String> =
            all_rules().iter().map(|r| r.id().to_string()).collect();

        let missing_in_js: Vec<_> = rust_ids.difference(&js_ids).collect();
        let missing_in_rust: Vec<_> = js_ids.difference(&rust_ids).collect();
        assert!(
            missing_in_js.is_empty() && missing_in_rust.is_empty(),
            "rule ID drift between Rust all_rules() and eslint plugin rules.js:\n\
             in Rust but not in JS: {missing_in_js:?}\n\
             in JS but not in Rust: {missing_in_rust:?}"
        );
    }

    #[test]
    fn rule_categories() {
        let rules = all_rules();
        let flk: Vec<_> = rules
            .iter()
            .filter(|r| r.category() == Category::Flakiness)
            .collect();
        let mnt: Vec<_> = rules
            .iter()
            .filter(|r| r.category() == Category::Maintenance)
            .collect();
        let str_: Vec<_> = rules
            .iter()
            .filter(|r| r.category() == Category::Structure)
            .collect();
        let dep: Vec<_> = rules
            .iter()
            .filter(|r| r.category() == Category::Dependencies)
            .collect();
        let val: Vec<_> = rules
            .iter()
            .filter(|r| r.category() == Category::Validation)
            .collect();
        assert_eq!(flk.len(), 5);
        assert_eq!(mnt.len(), 18);
        assert_eq!(str_.len(), 9);
        assert_eq!(dep.len(), 7);
        assert_eq!(val.len(), 14);
    }

    #[test]
    fn all_rule_names_correct() {
        let rules = all_rules();
        let expected = [
            ("VITEST-FLK-001", "TimeoutRule"),
            ("VITEST-FLK-002", "DateMockRule"),
            ("VITEST-FLK-003", "NetworkImportRule"),
            ("VITEST-FLK-004", "FakeTimersCleanupRule"),
            ("VITEST-FLK-005", "NonDeterministicRule"),
            ("VITEST-MNT-001", "NoAssertionRule"),
            ("VITEST-MNT-002", "MultipleExpectRule"),
            ("VITEST-MNT-003", "ConditionalLogicRule"),
            ("VITEST-MNT-004", "TryCatchRule"),
            ("VITEST-MNT-005", "EmptyTestRule"),
            ("VITEST-STR-001", "NestedDescribeRule"),
            ("VITEST-STR-002", "ReturnInTestRule"),
            ("VITEST-MNT-006", "MissingAwaitAssertionRule"),
            ("VITEST-MNT-007", "FocusedTestRule"),
            ("VITEST-MNT-008", "MissingMockCleanupRule"),
            ("VITEST-MNT-009", "WeakAssertionRule"),
            ("VITEST-MNT-010", "ImplementationCoupledRule"),
            ("VITEST-MNT-011", "TestIdNegativePresenceRule"),
            ("VITEST-DEP-001", "BannedModuleMockRule"),
            ("VITEST-DEP-002", "ProductionSingletonImportRule"),
            ("VITEST-DEP-003", "ResetEscapeHatchRule"),
            ("VITEST-DEP-004", "MockExportValidationRule"),
            ("VITEST-VAL-001", "ValidExpectRule"),
            ("VITEST-VAL-002", "ValidExpectInPromiseRule"),
            ("VITEST-VAL-003", "ValidDescribeCallbackRule"),
            ("VITEST-VAL-004", "ValidTitleRule"),
            ("VITEST-VAL-005", "NoUnneededAsyncExpectFunctionRule"),
            ("VITEST-NO-001", "NoStandaloneExpectRule"),
            ("VITEST-NO-002", "NoIdenticalTitleRule"),
            ("VITEST-NO-003", "NoCommentedOutTestsRule"),
            ("VITEST-NO-005", "NoTestPrefixesRule"),
            ("VITEST-NO-006", "NoDuplicateHooksRule"),
            ("VITEST-NO-007", "NoImportNodeTestRule"),
            ("VITEST-NO-008", "NoInterpolationInSnapshotsRule"),
            ("VITEST-NO-009", "NoLargeSnapshotsRule"),
            ("VITEST-NO-013", "NoDoneCallbackRule"),
            ("VITEST-NO-014", "NoConditionalExpectRule"),
            ("VITEST-PREF-001", "PreferToBeRule"),
            ("VITEST-PREF-002", "PreferToContainRule"),
            ("VITEST-PREF-003", "PreferToHaveLengthRule"),
            ("VITEST-PREF-005", "PreferSpyOnRule"),
            ("VITEST-PREF-007", "PreferCalledOnceRule"),
            ("VITEST-PREF-009", "PreferHooksOnTopRule"),
            ("VITEST-PREF-010", "PreferHooksInOrderRule"),
            ("VITEST-PREF-012", "PreferTodoRule"),
            ("VITEST-PREF-013", "PreferMockPromiseShorthandRule"),
            ("VITEST-PREF-014", "PreferExpectResolvesRule"),
            ("VITEST-REQ-001", "RequireHookRule"),
            ("VITEST-REQ-002", "RequireTopLevelDescribeRule"),
            ("VITEST-REQ-003", "RequireToThrowMessageRule"),
            ("VITEST-CON-001", "ConsistentTestItRule"),
            ("VITEST-CON-003", "ConsistentVitestViRule"),
            ("VITEST-CON-004", "HoistedApisOnTopRule"),
        ];
        for (id, name) in &expected {
            let rule = rules.iter().find(|r| r.id() == *id).unwrap();
            assert_eq!(
                rule.name(),
                *name,
                "Rule {} should have name '{}'",
                id,
                name
            );
        }
    }
}
