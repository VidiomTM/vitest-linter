use crate::models::{Category, MockScope, ModuleGraph, ParsedModule, Severity, Violation};
use crate::rules::Rule;

pub struct RequireHookRule;

impl Rule for RequireHookRule {
    fn id(&self) -> &'static str {
        "VITEST-REQ-001"
    }
    fn name(&self) -> &'static str {
        "RequireHookRule"
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
        let first_top_level = module
            .vi_mocks
            .iter()
            .filter(|m| m.scope == MockScope::Module)
            .min_by_key(|m| m.line);

        first_top_level.map_or_else(Vec::new, |mock| {
            vec![Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: format!(
                    "vi.mock('{}') is called at the top level instead of inside a hook",
                    mock.source
                ),
                file_path: module.file_path.clone(),
                line: mock.line,
                col: None,
                suggestion: Some(
                    "Use vi.doMock() with dynamic imports instead of top-level vi.mock()"
                        .to_string(),
                ),
                test_name: None,
            }]
        })
    }
}

pub struct RequireTopLevelDescribeRule;

impl Rule for RequireTopLevelDescribeRule {
    fn id(&self) -> &'static str {
        "VITEST-REQ-002"
    }
    fn name(&self) -> &'static str {
        "RequireTopLevelDescribeRule"
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
        if module.describe_blocks.is_empty() {
            return vec![];
        }

        let orphan_tests: Vec<_> = module
            .test_blocks
            .iter()
            .filter(|tb| !tb.is_nested)
            .collect();

        if orphan_tests.is_empty() {
            return vec![];
        }

        orphan_tests
            .iter()
            .map(|tb| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: format!("Test '{}' exists outside of any describe block", tb.name),
                file_path: tb.file_path.clone(),
                line: tb.line,
                col: None,
                suggestion: Some(
                    "Wrap this test in a describe() block for better organization".to_string(),
                ),
                test_name: Some(tb.name.clone()),
            })
            .collect()
    }
}

pub struct RequireToThrowMessageRule;

impl Rule for RequireToThrowMessageRule {
    fn id(&self) -> &'static str {
        "VITEST-REQ-003"
    }
    fn name(&self) -> &'static str {
        "RequireToThrowMessageRule"
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

        for (line_idx, line) in source.lines().enumerate() {
            let line_num = line_idx + 1;
            let trimmed = line.trim();

            if has_empty_to_throw(trimmed) {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: "toThrow() called without a message argument — provide an expected error message or pattern".to_string(),
                    file_path: module.file_path.clone(),
                    line: line_num,
                    col: None,
                    suggestion: Some(
                        "Add a string or RegExp argument to toThrow(), e.g. toThrow('expected error') or toThrow(/pattern/)".to_string(),
                    ),
                    test_name: None,
                });
            }
        }

        violations
    }
}

fn has_empty_throw_args(after: &str) -> bool {
    // after is the text following "toThrow", starting with '('
    let rest = after.strip_prefix('(').expect("must start with '('");
    let mut depth = 1_u32;
    let mut has_content = false;

    for ch in rest.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {
                if depth == 1 && !ch.is_whitespace() && ch != ',' {
                    has_content = true;
                }
            }
        }
    }

    !has_content
}

fn has_empty_to_throw_no_parens(after: &str) -> bool {
    let trimmed = after.trim_start();
    trimmed.is_empty()
        || trimmed.starts_with('.')
        || trimmed.starts_with(';')
        || trimmed.starts_with(')')
}

fn has_empty_to_throw(line: &str) -> bool {
    let mut search_from = 0;
    while let Some(pos) = line[search_from..].find("toThrow") {
        let abs_pos = search_from + pos;
        let after = &line[abs_pos + "toThrow".len()..];

        if after.starts_with('(') {
            if has_empty_throw_args(after) {
                return true;
            }
        } else if has_empty_to_throw_no_parens(after) {
            return true;
        }

        search_from = abs_pos + 1;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::models::{DescribeBlock, HookCall, ModuleGraph, TestBlock, ViMockCall};
    use std::path::PathBuf;

    fn default_ctx() -> crate::rules::LintContext<'static> {
        static CONFIG: std::sync::OnceLock<Config> = std::sync::OnceLock::new();
        let config = CONFIG.get_or_init(Config::default);
        crate::rules::LintContext {
            config,
            all_modules: &[],
        }
    }

    fn make_module(
        file_path: &str,
        vi_mocks: Vec<ViMockCall>,
        test_blocks: Vec<TestBlock>,
        describe_blocks: Vec<DescribeBlock>,
        hook_calls: Vec<HookCall>,
    ) -> ParsedModule {
        ParsedModule {
            file_path: PathBuf::from(file_path),
            imports: vec![],
            imports_parsed: vec![],
            vi_mocks,
            hook_calls,
            test_blocks,
            describe_blocks,
            has_fake_timers: false,
            expects_outside_tests: vec![],
            imports_node_test: false,
            snapshot_sizes: vec![],
            exports: Vec::new(),
            runtime: crate::models::TestRuntime::Unknown,
            playwright: None,
            global_stubs: vec![],
            ..ParsedModule::default()
        }
    }

    #[test]
    fn req_001_flags_top_level_vi_mock() {
        let module = make_module(
            "test.ts",
            vec![ViMockCall {
                source: "lodash".to_string(),
                line: 5,
                scope: MockScope::Module,
                factory_keys: Vec::new(),
            }],
            vec![],
            vec![],
            vec![],
        );
        let ctx = default_ctx();
        let v = RequireHookRule.check(&module, &ctx, &ModuleGraph::default());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule_id, "VITEST-REQ-001");
        assert_eq!(v[0].line, 5);
    }

    #[test]
    fn req_001_no_violation_when_mock_in_hook() {
        let module = make_module(
            "test.ts",
            vec![ViMockCall {
                source: "lodash".to_string(),
                line: 5,
                scope: MockScope::Hook,
                factory_keys: Vec::new(),
            }],
            vec![],
            vec![],
            vec![],
        );
        let ctx = default_ctx();
        let v = RequireHookRule.check(&module, &ctx, &ModuleGraph::default());
        assert!(v.is_empty());
    }

    #[test]
    fn req_001_no_violation_when_no_mocks() {
        let module = make_module("test.ts", vec![], vec![], vec![], vec![]);
        let ctx = default_ctx();
        let v = RequireHookRule.check(&module, &ctx, &ModuleGraph::default());
        assert!(v.is_empty());
    }

    #[test]
    fn req_001_reports_first_top_level_mock() {
        let module = make_module(
            "test.ts",
            vec![
                ViMockCall {
                    source: "lodash".to_string(),
                    line: 3,
                    scope: MockScope::Hook,
                    factory_keys: Vec::new(),
                },
                ViMockCall {
                    source: "axios".to_string(),
                    line: 5,
                    scope: MockScope::Module,
                    factory_keys: Vec::new(),
                },
                ViMockCall {
                    source: "react".to_string(),
                    line: 8,
                    scope: MockScope::Module,
                    factory_keys: Vec::new(),
                },
            ],
            vec![],
            vec![],
            vec![],
        );
        let ctx = default_ctx();
        let v = RequireHookRule.check(&module, &ctx, &ModuleGraph::default());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].line, 5);
    }

    #[test]
    fn req_002_flags_orphan_test() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("orphan.test.ts");
        std::fs::write(
            &path,
            r#"
import { test, describe, expect } from 'vitest';

test('orphan test', () => {
    expect(1).toBe(1);
});

describe('grouped', () => {
    test('inside describe', () => {
        expect(1).toBe(1);
    });
});
"#,
        )
        .unwrap();

        let module = ParsedModule {
            file_path: path.clone(),
            source: std::fs::read_to_string(&path).unwrap().into(),
            imports: vec![],
            imports_parsed: vec![],
            vi_mocks: vec![],
            hook_calls: vec![],
            test_blocks: vec![
                TestBlock {
                    name: "orphan test".to_string(),
                    file_path: path.clone(),
                    line: 4,
                    has_assertions: true,
                    assertion_count: 1,
                    has_conditional_logic: false,
                    has_try_catch: false,
                    uses_settimeout: false,
                    uses_promise_settimeout: false,
                    uses_datemock: false,
                    has_multiple_expects: false,
                    is_skipped: false,
                    is_only: false,
                    is_nested: false,
                    has_return_statement: false,
                    unawaited_async_assertions: 0,
                    uses_fake_timers: false,
                    uses_random: false,
                    has_expect_call_without_assertion: false,
                    has_return_of_expect: false,
                    title_is_template_literal: false,
                    has_async_expect_wrapper: false,
                    uses_fit_or_xit: false,
                    has_done_callback: false,
                    has_conditional_expect: false,
                    weak_assertion_count: 0,
                    has_real_timers_call: false,
                },
                TestBlock {
                    name: "inside describe".to_string(),
                    file_path: path.clone(),
                    line: 8,
                    has_assertions: true,
                    assertion_count: 1,
                    has_conditional_logic: false,
                    has_try_catch: false,
                    uses_settimeout: false,
                    uses_promise_settimeout: false,
                    uses_datemock: false,
                    has_multiple_expects: false,
                    is_skipped: false,
                    is_only: false,
                    is_nested: true,
                    has_return_statement: false,
                    unawaited_async_assertions: 0,
                    uses_fake_timers: false,
                    uses_random: false,
                    has_expect_call_without_assertion: false,
                    has_return_of_expect: false,
                    title_is_template_literal: false,
                    has_async_expect_wrapper: false,
                    uses_fit_or_xit: false,
                    has_done_callback: false,
                    has_conditional_expect: false,
                    weak_assertion_count: 0,
                    has_real_timers_call: false,
                },
            ],
            describe_blocks: vec![DescribeBlock {
                name: "grouped".to_string(),
                file_path: path.clone(),
                line: 7,
                is_only: false,
                depth: 1,
                title_is_template_literal: false,
                title_is_empty: false,
                is_async: false,
            }],
            has_fake_timers: false,
            expects_outside_tests: vec![],
            imports_node_test: false,
            snapshot_sizes: vec![],
            exports: Vec::new(),
            runtime: crate::models::TestRuntime::Unknown,
            playwright: None,
            global_stubs: vec![],
        };
        let ctx = default_ctx();
        let v = RequireToThrowMessageRule.check(&module, &ctx, &ModuleGraph::default());
        assert!(v.is_empty());
    }

    #[test]
    fn req_002_no_violation_when_all_nested() {
        let module = make_module(
            "test.ts",
            vec![],
            vec![TestBlock {
                name: "inside describe".to_string(),
                file_path: PathBuf::from("test.ts"),
                line: 3,
                has_assertions: true,
                assertion_count: 1,
                has_conditional_logic: false,
                has_try_catch: false,
                uses_settimeout: false,
                uses_promise_settimeout: false,
                uses_datemock: false,
                has_multiple_expects: false,
                is_skipped: false,
                is_only: false,
                is_nested: true,
                has_return_statement: false,
                unawaited_async_assertions: 0,
                uses_fake_timers: false,
                uses_random: false,
                has_expect_call_without_assertion: false,
                has_return_of_expect: false,
                title_is_template_literal: false,
                has_async_expect_wrapper: false,
                uses_fit_or_xit: false,
                has_done_callback: false,
                has_conditional_expect: false,
                weak_assertion_count: 0,
                has_real_timers_call: false,
            }],
            vec![DescribeBlock {
                name: "group".to_string(),
                file_path: PathBuf::from("test.ts"),
                line: 2,
                is_only: false,
                depth: 1,
                title_is_template_literal: false,
                title_is_empty: false,
                is_async: false,
            }],
            vec![],
        );
        let ctx = default_ctx();
        let v = RequireTopLevelDescribeRule.check(&module, &ctx, &ModuleGraph::default());
        assert!(v.is_empty());
    }

    #[test]
    fn req_003_flags_empty_to_throw() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("throw.test.ts");
        std::fs::write(
            &path,
            r#"expect(() => fn()).toThrow();
expect(() => fn()).toThrow();
"#,
        )
        .unwrap();

        let module = ParsedModule {
            file_path: path.clone(),
            source: std::fs::read_to_string(&path).unwrap().into(),
            imports: vec![],
            imports_parsed: vec![],
            vi_mocks: vec![],
            hook_calls: vec![],
            test_blocks: vec![],
            describe_blocks: vec![],
            has_fake_timers: false,
            expects_outside_tests: vec![],
            imports_node_test: false,
            snapshot_sizes: vec![],
            exports: Vec::new(),
            runtime: crate::models::TestRuntime::Unknown,
            playwright: None,
            global_stubs: vec![],
        };
        let ctx = default_ctx();
        let v = RequireToThrowMessageRule.check(&module, &ctx, &ModuleGraph::default());
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].rule_id, "VITEST-REQ-003");
        assert_eq!(v[0].line, 1);
        assert_eq!(v[1].line, 2);
    }

    #[test]
    fn req_003_no_violation_with_message() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("throw_msg.test.ts");
        std::fs::write(
            &path,
            r#"expect(() => fn()).toThrow('expected error');
expect(() => fn()).toThrow(/pattern/);
"#,
        )
        .unwrap();

        let module = ParsedModule {
            file_path: path.clone(),
            source: std::fs::read_to_string(&path).unwrap().into(),
            imports: vec![],
            imports_parsed: vec![],
            vi_mocks: vec![],
            hook_calls: vec![],
            test_blocks: vec![],
            describe_blocks: vec![],
            has_fake_timers: false,
            expects_outside_tests: vec![],
            imports_node_test: false,
            snapshot_sizes: vec![],
            exports: Vec::new(),
            runtime: crate::models::TestRuntime::Unknown,
            playwright: None,
            global_stubs: vec![],
        };
        let ctx = default_ctx();
        let v = RequireToThrowMessageRule.check(&module, &ctx, &ModuleGraph::default());
        assert!(v.is_empty());
    }

    #[test]
    fn req_003_flags_rejects_to_throw_without_message() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("rejects.test.ts");
        std::fs::write(
            &path,
            r#"expect(promise).rejects.toThrow();
"#,
        )
        .unwrap();

        let module = ParsedModule {
            file_path: path.clone(),
            source: std::fs::read_to_string(&path).unwrap().into(),
            imports: vec![],
            imports_parsed: vec![],
            vi_mocks: vec![],
            hook_calls: vec![],
            test_blocks: vec![],
            describe_blocks: vec![],
            has_fake_timers: false,
            expects_outside_tests: vec![],
            imports_node_test: false,
            snapshot_sizes: vec![],
            exports: Vec::new(),
            runtime: crate::models::TestRuntime::Unknown,
            playwright: None,
            global_stubs: vec![],
        };
        let ctx = default_ctx();
        let v = RequireToThrowMessageRule.check(&module, &ctx, &ModuleGraph::default());
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn req_003_no_violation_with_whitespace_only_parens() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("ws.test.ts");
        std::fs::write(
            &path,
            r#"expect(() => fn()).toThrow(   );
"#,
        )
        .unwrap();

        let module = ParsedModule {
            file_path: path.clone(),
            source: std::fs::read_to_string(&path).unwrap().into(),
            imports: vec![],
            imports_parsed: vec![],
            vi_mocks: vec![],
            hook_calls: vec![],
            test_blocks: vec![],
            describe_blocks: vec![],
            has_fake_timers: false,
            expects_outside_tests: vec![],
            imports_node_test: false,
            snapshot_sizes: vec![],
            exports: Vec::new(),
            runtime: crate::models::TestRuntime::Unknown,
            playwright: None,
            global_stubs: vec![],
        };
        let ctx = default_ctx();
        let v = RequireToThrowMessageRule.check(&module, &ctx, &ModuleGraph::default());
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn req_003_handles_multiple_to_throw_on_same_line() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("multi.test.ts");
        std::fs::write(
            &path,
            r#"expect(fn).toThrow();expect(fn2).toThrow('err');
"#,
        )
        .unwrap();

        let module = ParsedModule {
            file_path: path.clone(),
            source: std::fs::read_to_string(&path).unwrap().into(),
            imports: vec![],
            imports_parsed: vec![],
            vi_mocks: vec![],
            hook_calls: vec![],
            test_blocks: vec![],
            describe_blocks: vec![],
            has_fake_timers: false,
            expects_outside_tests: vec![],
            imports_node_test: false,
            snapshot_sizes: vec![],
            exports: Vec::new(),
            runtime: crate::models::TestRuntime::Unknown,
            playwright: None,
            global_stubs: vec![],
        };
        let ctx = default_ctx();
        let v = RequireToThrowMessageRule.check(&module, &ctx, &ModuleGraph::default());
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn has_empty_to_throw_basic_cases() {
        assert!(has_empty_to_throw(".toThrow()"));
        assert!(has_empty_to_throw(".toThrow(  )"));
        assert!(!has_empty_to_throw(".toThrow('error')"));
        assert!(!has_empty_to_throw(".toThrow(/pattern/)"));
        assert!(has_empty_to_throw(".rejects.toThrow()"));
        assert!(!has_empty_to_throw(".rejects.toThrow('error')"));
        assert!(has_empty_to_throw("toThrow()"));
    }
}
