use crate::models::{Category, HookKind, ModuleGraph, ParsedModule, Severity, Violation};
use crate::rules::Rule;

/// Flags tests that use `setTimeout`/`setInterval` without fake timers,
/// which can cause timing-dependent flakiness.
pub struct TimeoutRule;

impl Rule for TimeoutRule {
    fn id(&self) -> &'static str {
        "VITEST-FLK-001"
    }
    fn name(&self) -> &'static str {
        "TimeoutRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
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
            .filter(|tb| tb.uses_settimeout)
            .map(|tb| {
                let (message, suggestion) = if tb.uses_promise_settimeout {
                    ("setTimeout wrapped in Promise causes non-deterministic delays — use fake timers or auto-retry assertions".to_string(),
                     "Replace new Promise(r => setTimeout(r, N)) with vi.useFakeTimers() + vi.advanceTimersByTime() or auto-retry assertions".to_string())
                } else {
                    ("Test uses setTimeout which can cause timing-dependent flakiness".to_string(),
                     "Use vi.useFakeTimers() or vi.advanceTimersByTime() for deterministic time control".to_string())
                };
                Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message,
                    file_path: tb.file_path.clone(),
                    line: tb.line,
                    col: None,
                    suggestion: Some(suggestion),
                    test_name: Some(tb.name.clone()),
                }
            })
            .collect()
    }
}

/// Flags tests that use `Date` or `Date.now()` without `vi.useFakeTimers()`,
/// producing non-deterministic results across runs.
pub struct DateMockRule;

impl Rule for DateMockRule {
    fn id(&self) -> &'static str {
        "VITEST-FLK-002"
    }
    fn name(&self) -> &'static str {
        "DateMockRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        if module.has_fake_timers {
            return vec![];
        }
        module
            .test_blocks
            .iter()
            .filter(|tb| tb.uses_datemock)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: "Test uses Date without mocking, results may vary across runs".to_string(),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Use vi.useFakeTimers() to freeze time and ensure consistent test results"
                        .to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

/// Flags test files that import network libraries (axios, node-fetch, etc.)
/// without mocking, making tests susceptible to network failures.
pub struct NetworkImportRule;

const NETWORK_LIBS: &[&str] = &[
    "axios",
    "node-fetch",
    "got",
    "undici",
    "http",
    "https",
    "fetch",
];

impl Rule for NetworkImportRule {
    fn id(&self) -> &'static str {
        "VITEST-FLK-003"
    }
    fn name(&self) -> &'static str {
        "NetworkImportRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
    }
    fn check(
        &self,
        module: &ParsedModule,
        _ctx: &crate::rules::LintContext<'_>,
        _graph: &ModuleGraph,
    ) -> Vec<Violation> {
        let mut found = false;
        for imp in &module.imports_parsed {
            // Strip a `node:` prefix so built-ins like `node:http` match the
            // bare specifier list. Compare the full specifier exactly to avoid
            // false positives like `./utils/fetcher` or named imports whose
            // local binding textually contains a lib name (e.g. `HttpError`).
            let src = imp.source.trim_start_matches("node:");
            for lib in NETWORK_LIBS {
                if src == *lib {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        if !found {
            return vec![];
        }
        vec![Violation {
            rule_id: self.id().to_string(),
            rule_name: self.name().to_string(),
            severity: self.severity(),
            category: self.category(),
            message: "Test file imports network libraries — tests may fail due to network issues"
                .to_string(),
            file_path: module.file_path.clone(),
            line: 1,
            col: None,
            suggestion: Some("Mock network calls using vi.mock() or msw".to_string()),
            test_name: None,
        }]
    }
}

/// Flags tests that call `vi.useFakeTimers()` without a corresponding
/// `afterEach` cleanup, causing timer state to leak between tests.
pub struct FakeTimersCleanupRule;

const TIMER_CLEANUP_CALLS: &[&str] = &["vi.useRealTimers", "vi.clearAllTimers"];

impl Rule for FakeTimersCleanupRule {
    fn id(&self) -> &'static str {
        "VITEST-FLK-004"
    }
    fn name(&self) -> &'static str {
        "FakeTimersCleanupRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
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
            .filter(|tb| tb.uses_fake_timers)
            .filter(|tb| {
                // Skip if the test itself calls vi.useRealTimers() to clean up.
                if tb.has_real_timers_call {
                    return false;
                }
                // Check if any afterEach hook with timer cleanup covers this test.
                // A cleanup hook typically appears before the tests it protects.
                !module.hook_calls.iter().any(|h| {
                    h.kind == HookKind::AfterEach
                        && h.vi_calls
                            .iter()
                            .any(|c| TIMER_CLEANUP_CALLS.iter().any(|tc| c == tc))
                })
            })
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: format!(
                    "Test '{}' calls vi.useFakeTimers() without afterEach cleanup — timers will leak to other tests",
                    tb.name
                ),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Add afterEach(() => { vi.useRealTimers() }) or afterEach(() => { vi.clearAllTimers() }) to reset timers"
                        .to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

/// Flags tests that use `Math.random()` or `crypto.randomUUID()` without
/// seeding, producing non-deterministic results.
pub struct NonDeterministicRule;

impl Rule for NonDeterministicRule {
    fn id(&self) -> &'static str {
        "VITEST-FLK-005"
    }
    fn name(&self) -> &'static str {
        "NonDeterministicRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
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
            .filter(|tb| tb.uses_random)
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: format!(
                    "Test '{}' uses Math.random() or crypto.randomUUID() — results are non-deterministic",
                    tb.name
                ),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Mock Math.random() or use a seeded PRNG for deterministic tests".to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}
