use crate::models::{Category, ModuleGraph, ParsedModule, Severity, Violation};
use crate::rules::Rule;

pub struct ConsistentTestItRule;

impl Rule for ConsistentTestItRule {
    fn id(&self) -> &'static str {
        "VITEST-CON-001"
    }
    fn name(&self) -> &'static str {
        "ConsistentTestItRule"
    }
    fn severity(&self) -> Severity {
        Severity::Info
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

        let has_test = source.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("test(")
                || trimmed.starts_with("test.skip(")
                || trimmed.starts_with("test.only(")
                || trimmed.starts_with("test.todo(")
                || trimmed.starts_with("test.concurrent(")
                || trimmed.starts_with("test.each(")
                || trimmed.starts_with("test.describe(")
        });

        let has_it = source.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("it(")
                || trimmed.starts_with("it.skip(")
                || trimmed.starts_with("it.only(")
                || trimmed.starts_with("it.todo(")
                || trimmed.starts_with("it.concurrent(")
                || trimmed.starts_with("it.each(")
        });

        if has_test && has_it {
            vec![Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: "File mixes test() and it() — use one consistently".to_string(),
                file_path: module.file_path.clone(),
                line: 1,
                col: None,
                suggestion: Some(
                    "Standardize on either test() or it() throughout this file".to_string(),
                ),
                test_name: None,
            }]
        } else {
            vec![]
        }
    }
}

pub struct ConsistentVitestViRule;

impl Rule for ConsistentVitestViRule {
    fn id(&self) -> &'static str {
        "VITEST-CON-003"
    }
    fn name(&self) -> &'static str {
        "ConsistentVitestViRule"
    }
    fn severity(&self) -> Severity {
        Severity::Info
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
        let source = &module.source;

        let has_vi_import = module
            .imports_parsed
            .iter()
            .any(|imp| imp.source == "vitest" && imp.named.iter().any(|n| n == "vi"))
            || source.lines().any(|line| {
                let trimmed = line.trim();
                if !(trimmed.starts_with("import ") || trimmed.starts_with("import{"))
                    || !trimmed.contains("from")
                {
                    return false;
                }
                if !(trimmed.contains("'vitest'") || trimmed.contains("\"vitest\"")) {
                    return false;
                }
                // Match the `vi` named binding exactly, not substrings like
                // `visit` or `evaluate`. Accept `{ vi`, `, vi`, `vi,`, and `vi }`
                // so that multi-line and single-binding imports both work.
                let binding = trimmed.split("from").next().unwrap_or("");
                binding.split('{').nth(1).is_some_and(|inner| {
                    let inner = inner.split('}').next().unwrap_or("");
                    inner.split(',').any(|tok| {
                        tok.trim().strip_prefix("vi").is_some_and(|rest| {
                            rest.trim().is_empty() || rest.trim().starts_with(" as ")
                        })
                    })
                })
            });

        let has_vitest_namespace = module
            .imports_parsed
            .iter()
            .any(|imp| imp.source == "vitest" && imp.namespace.is_some())
            || source.lines().any(|line| {
                let trimmed = line.trim();
                trimmed.starts_with("import ")
                    && trimmed.contains("* as vitest")
                    && (trimmed.contains("'vitest'") || trimmed.contains("\"vitest\""))
            });

        if has_vi_import && has_vitest_namespace {
            vec![Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: "File imports both vi and vitest namespace — use one mocking import style"
                    .to_string(),
                file_path: module.file_path.clone(),
                line: 1,
                col: None,
                suggestion: Some(
                    "Use either `import { vi } from 'vitest'` or `import vitest from 'vitest'`, not both"
                        .to_string(),
                ),
                test_name: None,
            }]
        } else {
            vec![]
        }
    }
}

pub struct HoistedApisOnTopRule;

impl Rule for HoistedApisOnTopRule {
    fn id(&self) -> &'static str {
        "VITEST-CON-004"
    }
    fn name(&self) -> &'static str {
        "HoistedApisOnTopRule"
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
        let first_test_line = module.test_blocks.iter().map(|t| t.line).min();

        let first_describe_line = module.describe_blocks.iter().map(|d| d.line).min();

        let first_hook_line = module.hook_calls.iter().map(|h| h.line).min();

        let first_declaration_line = [first_test_line, first_describe_line, first_hook_line]
            .into_iter()
            .flatten()
            .min();

        let Some(earliest_line) = first_declaration_line else {
            return vec![];
        };

        module
            .vi_mocks
            .iter()
            .filter(|m| m.scope == crate::models::MockScope::Module)
            .filter(|m| m.line > earliest_line)
            .map(|m| Violation {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                category: self.category(),
                message: format!(
                    "vi.mock('{}') at line {} appears after test/describe/hook declarations — hoisted mocks should be at the top of the file",
                    m.source, m.line
                ),
                file_path: module.file_path.clone(),
                line: m.line,
                col: None,
                suggestion: Some(
                    "Move vi.mock() calls above all test(), describe(), and hook declarations"
                        .to_string(),
                ),
                test_name: None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    struct TempModule {
        _dir: tempfile::TempDir,
        module: ParsedModule,
    }

    fn make_module(content: &str, name: &str) -> TempModule {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        let module = ParsedModule {
            file_path: path,
            source: content.into(),
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
        TempModule { _dir: dir, module }
    }

    fn default_ctx() -> crate::rules::LintContext<'static> {
        use crate::config::Config;
        static CONFIG: std::sync::OnceLock<Config> = std::sync::OnceLock::new();
        let cfg = CONFIG.get_or_init(Config::default);
        crate::rules::LintContext {
            config: cfg,
            all_modules: &[],
        }
    }

    #[test]
    fn con_001_flags_mixed_test_it() {
        let tm = make_module(
            r#"
test('a', () => {});
it('b', () => {});
"#,
            "mixed.test.ts",
        );
        let v = ConsistentTestItRule.check(&tm.module, &default_ctx(), &ModuleGraph::default());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule_id, "VITEST-CON-001");
    }

    #[test]
    fn con_001_passes_test_only() {
        let tm = make_module(
            r#"
test('a', () => {});
test('b', () => {});
"#,
            "testonly.test.ts",
        );
        let v = ConsistentTestItRule.check(&tm.module, &default_ctx(), &ModuleGraph::default());
        assert!(v.is_empty());
    }

    #[test]
    fn con_001_passes_it_only() {
        let tm = make_module(
            r#"
it('a', () => {});
it('b', () => {});
"#,
            "itonly.test.ts",
        );
        let v = ConsistentTestItRule.check(&tm.module, &default_ctx(), &ModuleGraph::default());
        assert!(v.is_empty());
    }

    #[test]
    fn con_001_flags_mixed_skip_modifiers() {
        let tm = make_module(
            r#"
test.skip('a', () => {});
it('b', () => {});
"#,
            "skip.test.ts",
        );
        let v = ConsistentTestItRule.check(&tm.module, &default_ctx(), &ModuleGraph::default());
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn con_003_flags_mixed_vi_and_vitest_namespace() {
        let mut tm = make_module("", "mixed_vitest.test.ts");
        tm.module.imports_parsed.push(ImportEntry {
            source: "vitest".to_string(),
            named: vec!["vi".to_string()],
            default: None,
            namespace: Some("vitest".to_string()),
            line: 1,
        });
        let v = ConsistentVitestViRule.check(&tm.module, &default_ctx(), &ModuleGraph::default());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule_id, "VITEST-CON-003");
    }

    #[test]
    fn con_003_passes_vi_only() {
        let mut tm = make_module("", "vi_only.test.ts");
        tm.module.imports_parsed.push(ImportEntry {
            source: "vitest".to_string(),
            named: vec!["vi".to_string()],
            default: None,
            namespace: None,
            line: 1,
        });
        let v = ConsistentVitestViRule.check(&tm.module, &default_ctx(), &ModuleGraph::default());
        assert!(v.is_empty());
    }

    #[test]
    fn con_003_passes_vitest_namespace_only() {
        let mut tm = make_module("", "ns_only.test.ts");
        tm.module.imports_parsed.push(ImportEntry {
            source: "vitest".to_string(),
            named: vec![],
            default: None,
            namespace: Some("vitest".to_string()),
            line: 1,
        });
        let v = ConsistentVitestViRule.check(&tm.module, &default_ctx(), &ModuleGraph::default());
        assert!(v.is_empty());
    }

    #[test]
    fn con_004_flags_mock_after_test() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("hoist.test.ts");
        let mut tm = make_module("", "hoist.test.ts");
        tm.module.file_path = path;
        tm.module.test_blocks.push(TestBlock {
            name: "a".to_string(),
            file_path: tm.module.file_path.clone(),
            line: 5,
            has_assertions: false,
            assertion_count: 0,
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
        });
        tm.module.vi_mocks.push(ViMockCall {
            source: "./foo".to_string(),
            line: 10,
            scope: MockScope::Module,
            factory_keys: Vec::new(),
        });
        let _dir = dir;
        let v = HoistedApisOnTopRule.check(&tm.module, &default_ctx(), &ModuleGraph::default());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule_id, "VITEST-CON-004");
    }

    #[test]
    fn con_004_passes_mock_before_test() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("ok.test.ts");
        let mut tm = make_module("", "ok.test.ts");
        tm.module.file_path = path;
        tm.module.vi_mocks.push(ViMockCall {
            source: "./foo".to_string(),
            line: 3,
            scope: MockScope::Module,
            factory_keys: Vec::new(),
        });
        tm.module.test_blocks.push(TestBlock {
            name: "a".to_string(),
            file_path: tm.module.file_path.clone(),
            line: 10,
            has_assertions: false,
            assertion_count: 0,
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
        });
        let _dir = dir;
        let v = HoistedApisOnTopRule.check(&tm.module, &default_ctx(), &ModuleGraph::default());
        assert!(v.is_empty());
    }

    #[test]
    fn con_004_ignores_hook_scoped_mocks() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("hook.test.ts");
        let mut tm = make_module("", "hook.test.ts");
        tm.module.file_path = path;
        tm.module.test_blocks.push(TestBlock {
            name: "a".to_string(),
            file_path: tm.module.file_path.clone(),
            line: 5,
            has_assertions: false,
            assertion_count: 0,
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
        });
        tm.module.vi_mocks.push(ViMockCall {
            source: "./bar".to_string(),
            line: 10,
            scope: MockScope::Hook,
            factory_keys: Vec::new(),
        });
        let _dir = dir;
        let v = HoistedApisOnTopRule.check(&tm.module, &default_ctx(), &ModuleGraph::default());
        assert!(v.is_empty());
    }
}
