use std::fs;
use tempfile::TempDir;
use vitest_linter::engine::LintEngine;
use vitest_linter::parser::TsParser;
use vitest_linter::run_cli;

fn write_fixture(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, content).unwrap();
    path
}

fn parse(path: &std::path::Path) -> vitest_linter::models::ParsedModule {
    let parser = TsParser::new().unwrap();
    parser.parse_file(path).unwrap()
}

fn find_violation<'a>(
    violations: &'a [vitest_linter::models::Violation],
    rule_id: &str,
) -> Option<&'a vitest_linter::models::Violation> {
    violations.iter().find(|v| v.rule_id == rule_id)
}

#[test]
fn flk001_settimeout_triggers_rule() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "timeout.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

test('uses setTimeout', () => {
    setTimeout(() => {
        expect(true).toBe(true);
    }, 1000);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-FLK-001");
    assert!(v.is_some(), "Expected VITEST-FLK-001 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "TimeoutRule");
    assert_eq!(v.severity, vitest_linter::models::Severity::Warning);
    assert_eq!(v.category, vitest_linter::models::Category::Flakiness);
}

#[test]
fn flk002_date_without_fake_timers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "date.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

test('uses Date.now', () => {
    const now = Date.now();
    expect(now).toBeGreaterThan(0);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-FLK-002");
    assert!(v.is_some(), "Expected VITEST-FLK-002 violation");
    assert_eq!(v.unwrap().rule_name, "DateMockRule");
}

#[test]
fn flk002_date_with_fake_timers_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "date_safe.test.ts",
        r#"
import { describe, test, expect, vi } from 'vitest';

test('uses Date.now with fake timers', () => {
    vi.useFakeTimers();
    const now = Date.now();
    expect(now).toBeGreaterThan(0);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-FLK-002").is_none(),
        "Should not trigger VITEST-FLK-002 when useFakeTimers is present"
    );
}

#[test]
fn flk003_network_import_axios() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "network.test.ts",
        r#"
import axios from 'axios';
import { test, expect } from 'vitest';

test('fetches data', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-FLK-003");
    assert!(v.is_some(), "Expected VITEST-FLK-003 violation");
    assert_eq!(v.unwrap().rule_name, "NetworkImportRule");
}

#[test]
fn flk003_network_import_no_false_positive_on_substring() {
    // A relative import whose path merely contains "http" as a substring
    // (e.g. `./utils/https-helper`) must NOT trigger the network rule.
    // The source specifier must match a known network library exactly.
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "no-network.test.ts",
        r#"
import { httpsHelper } from './utils/https-helper';
import { test, expect } from 'vitest';

test('uses helper', () => {
    expect(httpsHelper).toBeDefined();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-FLK-003");
    assert!(
        v.is_none(),
        "Relative import './utils/https-helper' must NOT trigger VITEST-FLK-003"
    );
}

#[test]
fn mnt001_no_assertions() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "no_assert.test.ts",
        r#"
import { test } from 'vitest';

test('does nothing', () => {
    const x = 1 + 1;
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-001");
    assert!(v.is_some(), "Expected VITEST-MNT-001 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "NoAssertionRule");
    assert_eq!(v.severity, vitest_linter::models::Severity::Error);
}

#[test]
fn mnt002_too_many_assertions() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "many_asserts.test.ts",
        r#"
import { test, expect } from 'vitest';

test('too many assertions', () => {
    expect(1).toBe(1);
    expect(2).toBe(2);
    expect(3).toBe(3);
    expect(4).toBe(4);
    expect(5).toBe(5);
    expect(6).toBe(6);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-002");
    assert!(v.is_some(), "Expected VITEST-MNT-002 violation");
    assert_eq!(v.unwrap().rule_name, "MultipleExpectRule");
}

#[test]
fn mnt002_exactly_five_assertions_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "five_asserts.test.ts",
        r#"
import { test, expect } from 'vitest';

test('exactly five assertions', () => {
    expect(1).toBe(1);
    expect(2).toBe(2);
    expect(3).toBe(3);
    expect(4).toBe(4);
    expect(5).toBe(5);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-MNT-002").is_none(),
        "Exactly 5 assertions should NOT trigger MNT-002 (threshold is > 5)"
    );
}

#[test]
fn mnt003_conditional_logic() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "conditional.test.ts",
        r#"
import { test, expect } from 'vitest';

test('has conditional', () => {
    const x = Math.random();
    if (x > 0.5) {
        expect(true).toBe(true);
    } else {
        expect(false).toBe(false);
    }
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-003");
    assert!(v.is_some(), "Expected VITEST-MNT-003 violation");
    assert_eq!(v.unwrap().rule_name, "ConditionalLogicRule");
}

#[test]
fn mnt004_try_catch() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "trycatch.test.ts",
        r#"
import { test, expect } from 'vitest';

test('has try catch', () => {
    try {
        JSON.parse('invalid');
    } catch (e) {
        expect(e).toBeDefined();
    }
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-004");
    assert!(v.is_some(), "Expected VITEST-MNT-004 violation");
    assert_eq!(v.unwrap().rule_name, "TryCatchRule");
}

#[test]
fn mnt005_skipped_test() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "skipped.test.ts",
        r#"
import { test, expect } from 'vitest';

test.skip('is skipped', () => {
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-005");
    assert!(v.is_some(), "Expected VITEST-MNT-005 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "EmptyTestRule");
    assert_eq!(v.severity, vitest_linter::models::Severity::Info);
}

#[test]
fn str001_deeply_nested_describe() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "deeply_nested.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

describe('level1', () => {
    describe('level2', () => {
        describe('level3', () => {
            describe('level4', () => {
                test('deeply nested', () => {
                    expect(true).toBe(true);
                });
            });
        });
    });
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-STR-001");
    assert!(
        v.is_some(),
        "Expected VITEST-STR-001 violation for 4-level nesting"
    );
    assert_eq!(v.unwrap().rule_name, "NestedDescribeRule");
}

#[test]
fn str002_return_in_test() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "return.test.ts",
        r#"
import { test, expect } from 'vitest';

test('has return', () => {
    const result = 1 + 1;
    expect(result).toBe(2);
    return result;
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-STR-002");
    assert!(v.is_some(), "Expected VITEST-STR-002 violation");
    assert_eq!(v.unwrap().rule_name, "ReturnInTestRule");
}

#[test]
fn parser_detects_imports() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "imports.test.ts",
        r#"
import { test, expect } from 'vitest';
import axios from 'axios';

test('simple', () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert!(module.imports.len() >= 2);
    assert!(module.imports.iter().any(|i| i.contains("axios")));
}

#[test]
fn parser_detects_test_blocks() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "blocks.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

test('first', () => {
    expect(1).toBe(1);
});

test('second', () => {
    expect(2).toBe(2);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 2);
}

#[test]
fn clean_file_has_no_violations() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "clean.test.ts",
        r#"
import { test, expect } from 'vitest';

test('clean test', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        violations.is_empty(),
        "Clean file should have no violations: {:?}",
        violations
    );
}

#[test]
fn lint_dir_discovers_files() {
    let dir = TempDir::new().unwrap();
    write_fixture(
        &dir,
        "a.test.ts",
        r#"
import { test, expect } from 'vitest';
test('a', () => { expect(1).toBe(1); });
"#,
    );
    write_fixture(
        &dir,
        "b.test.ts",
        r#"
import { test, expect } from 'vitest';
test('b', () => { expect(1).toBe(1); });
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[dir.path().to_path_buf()]).unwrap();
    assert!(
        violations.is_empty(),
        "Clean files should have no violations"
    );
}

#[test]
fn engine_ignores_non_test_files() {
    let dir = TempDir::new().unwrap();
    write_fixture(
        &dir,
        "utils.ts",
        r#"
export function add(a: number, b: number) { return a + b; }
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[dir.path().to_path_buf()]).unwrap();
    assert!(violations.is_empty());
}

#[test]
fn engine_ignores_non_test_files_with_test_content() {
    let dir = TempDir::new().unwrap();
    write_fixture(
        &dir,
        "helper.ts",
        r#"
import { test } from 'vitest';
test('no assert', () => { const x = 1; });
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[dir.path().to_path_buf()]).unwrap();
    assert!(
        violations.is_empty(),
        "Non-test files should be ignored even if they contain test-like code"
    );
}

#[test]
fn new_date_triggers_datemock() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "newdate.test.ts",
        r#"
import { test, expect } from 'vitest';

test('uses new Date', () => {
    const d = new Date();
    expect(d).toBeDefined();
});
"#,
    );
    let module = parse(&path);
    assert!(
        module.test_blocks[0].uses_datemock,
        "new Date() should set uses_datemock"
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(find_violation(&violations, "VITEST-FLK-002").is_some());
}

#[test]
fn cli_json_format_with_error() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "no_assert.test.ts",
        r#"
import { test } from 'vitest';
test('no assert', () => { const x = 1; });
"#,
    );
    let output_path = dir.path().join("output.json");
    let has_errors = run_cli(
        &[test_path],
        "json",
        Some(&output_path),
        true,
        false,
        true,
        "HEAD",
    )
    .unwrap();

    assert!(has_errors, "Should have errors (MNT-001 is Error severity)");

    let json = fs::read_to_string(&output_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_array(), "JSON output should be an array");
    let arr = parsed.as_array().unwrap();
    assert!(arr
        .iter()
        .any(|v| v["rule_id"].as_str() == Some("VITEST-MNT-001")));
    assert!(arr
        .iter()
        .any(|v| v["rule_name"].as_str() == Some("NoAssertionRule")));
}

#[test]
fn cli_json_format_clean_file() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "clean.test.ts",
        r#"
import { test, expect } from 'vitest';
test('clean', () => { expect(1).toBe(1); });
"#,
    );
    let output_path = dir.path().join("output.json");
    let has_errors = run_cli(
        &[test_path],
        "json",
        Some(&output_path),
        true,
        false,
        true,
        "HEAD",
    )
    .unwrap();

    assert!(!has_errors, "Clean file should have no errors");

    let json = fs::read_to_string(&output_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.as_array().unwrap().is_empty());
}

#[test]
fn cli_terminal_severity_counts() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "mixed.test.ts",
        r#"
import { test, expect } from 'vitest';

test.skip('skipped', () => {});
test('cond', () => { if (true) { expect(1).toBe(1); } });
"#,
    );
    let output_path = dir.path().join("output.txt");
    let has_errors = run_cli(
        &[test_path],
        "terminal",
        Some(&output_path),
        true,
        false,
        true,
        "HEAD",
    )
    .unwrap();

    assert!(has_errors, "Should have errors from MNT-001");

    let output = fs::read_to_string(&output_path).unwrap();
    assert!(
        output.contains("1 error(s)"),
        "Expected 1 error in output, got: {}",
        output
    );
    assert!(
        output.contains("2 warning(s)"),
        "Expected 2 warnings in output, got: {}",
        output
    );
    assert!(
        output.contains("1 info"),
        "Expected 1 info in output, got: {}",
        output
    );
}

#[test]
fn cli_terminal_clean_file() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "clean.test.ts",
        r#"
import { test, expect } from 'vitest';
test('clean', () => { expect(1).toBe(1); });
"#,
    );
    let output_path = dir.path().join("output.txt");
    let has_errors = run_cli(
        &[test_path],
        "terminal",
        Some(&output_path),
        true,
        false,
        true,
        "HEAD",
    )
    .unwrap();

    assert!(!has_errors);
    let output = fs::read_to_string(&output_path).unwrap();
    assert!(output.contains("No test smells detected"));
}

#[test]
fn cli_binary_exits_with_error_for_violations() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "no_assert.test.ts",
        r#"
import { test } from 'vitest';
test('no assert', () => { const x = 1; });
"#,
    );

    let bin = std::env::var("CARGO_BIN_EXE_vitest-linter").unwrap();
    let output = std::process::Command::new(&bin)
        .arg(&test_path)
        .arg("--no-color")
        .arg("--unstable-rules")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "Binary should exit with error code when violations found"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("VITEST-MNT-001"),
        "Output should contain MNT-001 violation"
    );
}

#[test]
fn cli_binary_clean_exit() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "clean.test.ts",
        r#"
import { test, expect } from 'vitest';
test('clean', () => { expect(1).toBe(1); });
"#,
    );

    let bin = std::env::var("CARGO_BIN_EXE_vitest-linter").unwrap();
    let output = std::process::Command::new(&bin)
        .arg(&test_path)
        .arg("--no-color")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Binary should exit successfully for clean files"
    );
}

#[test]
fn parser_it_block() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "it.test.ts",
        r#"
import { it, expect } from 'vitest';

it('works', () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
    assert_eq!(module.test_blocks[0].name, "works");
    assert!(module.test_blocks[0].has_assertions);
}

#[test]
fn parser_test_todo_is_skipped() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "todo.test.ts",
        r#"
import { test, expect } from 'vitest';

test.todo('pending', () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
    assert!(module.test_blocks[0].is_skipped);
}

#[test]
fn parser_template_string_name() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "template.test.ts",
        r#"
import { test, expect } from 'vitest';

test(`template name`, () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
    assert!(module.test_blocks[0].has_assertions);
}

#[test]
fn mnt007_test_only_detected() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "only.test.ts",
        r#"
import { test, expect } from 'vitest';

test.only('focused test', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-007");
    assert!(
        v.is_some(),
        "Expected VITEST-MNT-007 violation for test.only"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "FocusedTestRule");
    assert_eq!(v.severity, vitest_linter::models::Severity::Error);
}

#[test]
fn mnt007_describe_only_detected() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "describe_only.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

describe.only('focused suite', () => {
    test('inside', () => {
        expect(1).toBe(1);
    });
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-007");
    assert!(
        v.is_some(),
        "Expected VITEST-MNT-007 violation for describe.only"
    );
}

#[test]
fn mnt007_it_only_detected() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "it_only.test.ts",
        r#"
import { it, expect } from 'vitest';

it.only('focused it', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-007");
    assert!(v.is_some(), "Expected VITEST-MNT-007 violation for it.only");
}

#[test]
fn mnt007_no_false_positive_without_only() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "normal.test.ts",
        r#"
import { test, expect } from 'vitest';

test('normal test', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-MNT-007").is_none(),
        "Should not trigger MNT-007 without .only"
    );
}

#[test]
fn suppression_disable_next_line() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "suppressed.test.ts",
        r#"
import { test, expect } from 'vitest';

// vitest-linter-disable-next-line VITEST-FLK-001
test('has timeout', () => {
    setTimeout(() => {
        expect(1).toBe(1);
    }, 1000);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-FLK-001").is_none(),
        "VITEST-FLK-001 should be suppressed by disable-next-line comment"
    );
}

#[test]
fn suppression_disable_next_line_all_rules() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "suppressed_all.test.ts",
        r#"
import { test } from 'vitest';

// vitest-linter-disable-next-line
test('no assert', () => {
    const x = 1;
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-MNT-001").is_none(),
        "MNT-001 should be suppressed by disable-next-line with no rule ID"
    );
}

#[test]
fn suppression_disable_range() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "range_suppressed.test.ts",
        r#"
import { test, expect } from 'vitest';

// vitest-linter-disable VITEST-FLK-001
test('has timeout', () => {
    setTimeout(() => {
        expect(1).toBe(1);
    }, 1000);
});
// vitest-linter-enable VITEST-FLK-001
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-FLK-001").is_none(),
        "VITEST-FLK-001 should be suppressed by disable/enable range"
    );
}

#[test]
fn suppression_does_not_affect_other_rules() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "partial_suppressed.test.ts",
        r#"
import { test, expect } from 'vitest';

// vitest-linter-disable-next-line VITEST-FLK-001
test('has timeout but no assertions', () => {
    setTimeout(() => {
        const x = 1;
    }, 1000);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-FLK-001").is_none(),
        "VITEST-FLK-001 should be suppressed"
    );
    // MNT-001 (no assertions) should still fire
    assert!(
        find_violation(&violations, "VITEST-MNT-001").is_some(),
        "VITEST-MNT-001 should fire even when FLK-001 is suppressed"
    );
}

#[test]
fn flk004_fake_timers_without_cleanup() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "fake_timers_no_cleanup.test.ts",
        r#"
import { test, expect, vi } from 'vitest';

test('uses fake timers', () => {
    vi.useFakeTimers();
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-FLK-004");
    assert!(v.is_some(), "Expected VITEST-FLK-004 violation");
    assert_eq!(v.unwrap().rule_name, "FakeTimersCleanupRule");
}

#[test]
fn flk004_fake_timers_with_after_each_cleanup() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "fake_timers_safe.test.ts",
        r#"
import { test, expect, vi, afterEach } from 'vitest';

afterEach(() => {
    vi.useRealTimers();
});

test('uses fake timers safely', () => {
    vi.useFakeTimers();
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-FLK-004").is_none(),
        "Should not trigger FLK-004 when afterEach has vi.useRealTimers()"
    );
}

#[test]
fn flk004_fake_timers_with_use_real_timers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "fake_timers_real.test.ts",
        r#"
import { test, expect, vi, afterEach } from 'vitest';

afterEach(() => {
    vi.useRealTimers();
});

test('uses fake timers', () => {
    vi.useFakeTimers();
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-FLK-004").is_none(),
        "Should not trigger FLK-004 when afterEach has vi.useRealTimers()"
    );
}

#[test]
fn flk004_fake_timers_with_non_cleanup_after_each() {
    // An afterEach that does NOT call a timer-cleanup method must not suppress
    // the violation. This guards the && / == logic in FakeTimersCleanupRule.
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "fake_timers_unrelated_after_each.test.ts",
        r#"
import { test, expect, vi, afterEach } from 'vitest';

afterEach(() => {
    vi.clearAllMocks();
});

test('uses fake timers', () => {
    vi.useFakeTimers();
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-FLK-004").is_some(),
        "Should trigger FLK-004 when afterEach has no timer cleanup"
    );
}

#[test]
fn mnt008_mock_without_cleanup() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "mock_no_cleanup.test.ts",
        r#"
import { vi, test, expect } from 'vitest';

vi.mock('./some-module', () => ({ default: {} }));

test('uses mock', () => {
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-008");
    assert!(v.is_some(), "Expected VITEST-MNT-008 violation");
    assert_eq!(v.unwrap().rule_name, "MissingMockCleanupRule");
}

#[test]
fn mnt008_mock_with_cleanup_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "mock_with_cleanup.test.ts",
        r#"
import { vi, test, expect, afterEach } from 'vitest';

vi.mock('./some-module', () => ({ default: {} }));

afterEach(() => {
    vi.restoreAllMocks();
});

test('uses mock safely', () => {
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-MNT-008").is_none(),
        "Should not trigger MNT-008 when afterEach has vi.restoreAllMocks()"
    );
}

#[test]
fn mnt008_mock_with_before_each_cleanup_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "mock_before_each_cleanup.test.ts",
        r#"
import { vi, test, expect, beforeEach } from 'vitest';

vi.mock('./some-module', () => ({ default: {} }));

beforeEach(() => {
    vi.clearAllMocks();
});

test('uses mock safely', () => {
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-MNT-008").is_none(),
        "Should not trigger MNT-008 when beforeEach has vi.clearAllMocks()"
    );
}

#[test]
fn mnt008_no_mock_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "no_mock.test.ts",
        r#"
import { test, expect } from 'vitest';

test('no mocks', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-MNT-008").is_none(),
        "Should not trigger MNT-008 without vi.mock()"
    );
}

#[test]
fn str001_shallow_nesting_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "shallow_nesting.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

describe('outer', () => {
    describe('inner', () => {
        test('shallow', () => {
            expect(true).toBe(true);
        });
    });
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-STR-001").is_none(),
        "2-level nesting should NOT trigger STR-001 (threshold is > 3)"
    );
}

#[test]
fn parser_switch_statement_conditional() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "switch.test.ts",
        r#"
import { test, expect } from 'vitest';

test('has switch', () => {
    const x = 1;
    switch (x) {
        case 1:
            expect(x).toBe(1);
            break;
        default:
            expect(x).toBe(0);
    }
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
    assert!(module.test_blocks[0].has_conditional_logic);
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(find_violation(&violations, "VITEST-MNT-003").is_some());
}

#[test]
fn parser_describe_without_callback() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "describe_no_cb.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

describe('no callback');

test('outside', () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
    assert_eq!(module.test_blocks[0].name, "outside");
    assert!(module.test_blocks[0].has_assertions);
}

#[test]
fn parser_describe_with_non_function_callback() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "describe_string_cb.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

describe('name', 'not a function');

test('outside', () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
}

#[test]
fn parser_test_non_string_name() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "nonstr.test.ts",
        r#"
import { test, expect } from 'vitest';

const testName = 'dynamic';
test(testName, () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    // Dynamic (non-string-literal) test names are retained rather than
    // silently dropped — the name falls back to the raw identifier text.
    assert_eq!(module.test_blocks.len(), 1);
    assert_eq!(module.test_blocks[0].name, "testName");
}

#[test]
fn parser_arrow_function_without_block_body() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "arrow_expr.test.ts",
        r#"
import { test, expect } from 'vitest';

test('arrow expr', () => expect(1).toBe(1));
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
    assert!(module.test_blocks[0].has_assertions);
}

#[test]
fn parser_empty_file() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "empty.test.ts", "");
    let module = parse(&path);
    assert!(module.test_blocks.is_empty());
    assert!(module.imports.is_empty());
    assert!(!module.has_fake_timers);
}

#[test]
fn parser_file_with_only_imports() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "only_imports.test.ts",
        r#"
import { test, expect } from 'vitest';
import * as utils from './utils';
"#,
    );
    let module = parse(&path);
    assert!(module.test_blocks.is_empty());
    assert_eq!(module.imports.len(), 2);
}

#[test]
fn parser_syntax_error_file() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "bad_syntax.test.ts",
        r#"
import { test, expect } from 'vitest';
test('bad syntax' () => {
    expect(1).toBe(1);
});
"#,
    );
    let parser = TsParser::new().unwrap();
    let result = parser.parse_file(&path);
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
    let output = result.unwrap();
    assert!(output.test_blocks.is_empty());
}

#[test]
fn parser_it_skip() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "it_skip.test.ts",
        r#"
import { it, expect } from 'vitest';

it.skip('skipped it', () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
    assert!(module.test_blocks[0].is_skipped);
}

#[test]
fn parser_expect_in_call_args() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "expect_args.test.ts",
        r#"
import { test, expect } from 'vitest';

test('expect in args', () => {
    const fn = (val: number) => val;
    fn(expect(1).toBe(1));
});
"#,
    );
    let module = parse(&path);
    assert!(module.test_blocks[0].has_assertions);
}

#[test]
fn parser_new_expression_non_date() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "new_expr.test.ts",
        r#"
import { test, expect } from 'vitest';

test('new expression', () => {
    const arr = new Array(5);
    expect(arr.length).toBe(5);
});
"#,
    );
    let module = parse(&path);
    assert!(!module.test_blocks[0].uses_datemock);
}

#[test]
fn parser_file_without_fake_timers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "no_fake.test.ts",
        r#"
import { test, expect } from 'vitest';

test('normal', () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert!(!module.has_fake_timers);
}

#[test]
fn parser_call_expression_without_function_field() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "no_func.test.ts",
        r#"
import { test, expect } from 'vitest';

const fn = () => {};
fn();

test('works', () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
}

#[test]
fn engine_empty_directory() {
    let dir = TempDir::new().unwrap();
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[dir.path().to_path_buf()]).unwrap();
    assert!(violations.is_empty());
}

#[test]
fn engine_directory_with_mixed_files() {
    let dir = TempDir::new().unwrap();
    write_fixture(
        &dir,
        "utils.ts",
        r#"export function add(a: number, b: number) { return a + b; }"#,
    );
    write_fixture(
        &dir,
        "real.test.ts",
        r#"
import { test, expect } from 'vitest';
test('clean', () => { expect(1).toBe(1); });
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[dir.path().to_path_buf()]).unwrap();
    assert!(violations.is_empty());
    assert_eq!(violations.len(), 0);
}

#[test]
fn cli_terminal_with_suggestion() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "timeout.test.ts",
        r#"
import { test, expect } from 'vitest';
test('timeout', () => {
    setTimeout(() => { expect(1).toBe(1); }, 1000);
});
"#,
    );
    let output_path = dir.path().join("output.txt");
    let has_errors = run_cli(
        &[test_path],
        "terminal",
        Some(&output_path),
        true,
        false,
        true,
        "HEAD",
    )
    .unwrap();
    assert!(!has_errors);
    let output = fs::read_to_string(&output_path).unwrap();
    assert!(output.contains("Suggestion:"));
}

#[test]
fn cli_json_to_stdout() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "no_assert.test.ts",
        r#"
import { test } from 'vitest';
test('no assert', () => { const x = 1; });
"#,
    );
    let has_errors = run_cli(&[test_path], "json", None, true, false, true, "HEAD").unwrap();
    assert!(has_errors);
}

#[test]
fn cli_terminal_no_color_flag() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "clean.test.ts",
        r#"
import { test, expect } from 'vitest';
test('clean', () => { expect(1).toBe(1); });
"#,
    );
    let output_path = dir.path().join("output.txt");
    run_cli(
        &[test_path],
        "terminal",
        Some(&output_path),
        true,
        false,
        true,
        "HEAD",
    )
    .unwrap();
    let output = fs::read_to_string(&output_path).unwrap();
    assert!(output.contains("No test smells detected"));
}

#[test]
fn parser_it_todo_is_skipped() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "it_todo.test.ts",
        r#"
import { it, expect } from 'vitest';

it.todo('pending it', () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
    assert!(module.test_blocks[0].is_skipped);
}

#[test]
fn parser_single_quote_name() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "singlequote.test.ts",
        r#"
import { test, expect } from 'vitest';

test('single quote', () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
    assert_eq!(module.test_blocks[0].name, "single quote");
}

#[test]
fn parser_double_quote_name() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "dblquote.test.ts",
        r#"
import { test, expect } from 'vitest';

test("double quoted name", () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
    assert_eq!(module.test_blocks[0].name, "double quoted name");
    assert!(module.test_blocks[0].has_assertions);
}

#[test]
fn cli_terminal_with_color_enabled_violations() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "no_assert.test.ts",
        r#"
import { test } from 'vitest';
test('no assert', () => { const x = 1; });
"#,
    );
    let output_path = dir.path().join("output.txt");
    let has_errors = run_cli(
        &[test_path],
        "terminal",
        Some(&output_path),
        false,
        false,
        true,
        "HEAD",
    )
    .unwrap();
    assert!(has_errors);
    let output = fs::read_to_string(&output_path).unwrap();
    assert!(output.contains("Found"));
    assert!(output.contains("1 error(s)"));
}

#[test]
fn cli_terminal_with_color_enabled() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "clean.test.ts",
        r#"
import { test, expect } from 'vitest';
test('clean', () => { expect(1).toBe(1); });
"#,
    );
    let output_path = dir.path().join("output.txt");
    let has_errors = run_cli(
        &[test_path],
        "terminal",
        Some(&output_path),
        false,
        false,
        true,
        "HEAD",
    )
    .unwrap();
    assert!(!has_errors);
    let output = fs::read_to_string(&output_path).unwrap();
    assert!(output.contains("No test smells detected"));
}

#[test]
fn engine_non_test_file_as_path() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "utils.ts",
        r#"export function add(a: number, b: number) { return a + b; }"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(violations.is_empty());
}

#[test]
fn engine_nonexistent_path() {
    let dir = TempDir::new().unwrap();
    let fake_path = dir.path().join("nonexistent.test.ts");
    let engine = LintEngine::new(true).unwrap();
    let result = engine.lint_paths(&[fake_path]);
    assert!(result.is_ok());
}

#[test]
fn parser_test_no_args() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "noargs.test.ts",
        r#"
import { test } from 'vitest';

test();
"#,
    );
    let module = parse(&path);
    assert!(module.test_blocks.is_empty());
}

#[test]
fn parser_describe_with_single_arg() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "describe_one_arg.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

describe('only name');

test('outside', () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
    assert_eq!(module.test_blocks[0].name, "outside");
}

#[test]
fn parser_iife_call_expression() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "iife.test.ts",
        r#"
import { test, expect } from 'vitest';

(function() {
    const x = 1;
})();

test('works', () => {
    expect(1).toBe(1);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
}

#[test]
fn parser_nested_function_calls_in_test() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "nested_calls.test.ts",
        r#"
import { test, expect } from 'vitest';

test('nested', () => {
    function helper() {
        return expect(1).toBe(1);
    }
    helper();
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
    assert!(module.test_blocks[0].has_assertions);
}

#[test]
fn cli_terminal_violations_without_suggestion_path() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "mixed_violations.test.ts",
        r#"
import { test, expect } from 'vitest';

test('has if', () => {
    if (true) {
        expect(1).toBe(1);
    }
});
"#,
    );
    let output_path = dir.path().join("output.txt");
    run_cli(
        &[test_path],
        "terminal",
        Some(&output_path),
        true,
        false,
        true,
        "HEAD",
    )
    .unwrap();
    let output = fs::read_to_string(&output_path).unwrap();
    assert!(output.contains("Found"));
}

#[test]
fn cli_json_to_stdout_clean() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "clean.test.ts",
        r#"
import { test, expect } from 'vitest';
test('clean', () => { expect(1).toBe(1); });
"#,
    );
    let has_errors = run_cli(&[test_path], "json", None, true, false, true, "HEAD").unwrap();
    assert!(!has_errors);
}

#[test]
fn cli_terminal_to_stdout_with_violations() {
    let dir = TempDir::new().unwrap();
    let test_path = write_fixture(
        &dir,
        "no_assert.test.ts",
        r#"
import { test } from 'vitest';
test('no assert', () => { const x = 1; });
"#,
    );
    let has_errors = run_cli(&[test_path], "terminal", None, true, false, true, "HEAD").unwrap();
    assert!(has_errors);
}

#[test]
fn parser_date_now_in_test() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "date_now.test.ts",
        r#"
import { test, expect } from 'vitest';

test('date now', () => {
    const now = Date.now();
    expect(now).toBeGreaterThan(0);
});
"#,
    );
    let module = parse(&path);
    assert!(module.test_blocks[0].uses_datemock);
    assert!(!module.has_fake_timers);
}

#[test]
fn parser_settimeout_in_test() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "settimeout.test.ts",
        r#"
import { test, expect } from 'vitest';

test('timeout', () => {
    setTimeout(() => {
        expect(1).toBe(1);
    }, 100);
});
"#,
    );
    let module = parse(&path);
    assert!(module.test_blocks[0].uses_settimeout);
}

#[test]
fn parser_test_each_pattern() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "test_each.test.ts",
        r#"
import { test, expect } from 'vitest';

test.each([[1, 1], [2, 4]])('square of %i is %i', (input, expected) => {
    expect(input * input).toBe(expected);
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 0);
}

#[test]
fn parser_describe_each_pattern() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "describe_each.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

describe.each(['a', 'b'])('letter %s', (letter) => {
    test('exists', () => {
        expect(letter).toBeDefined();
    });
});
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
    assert_eq!(module.test_blocks[0].name, "exists");
}

#[test]
fn parser_chained_member_call() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "chained.test.ts",
        r#"
import { test, expect } from 'vitest';

test('chained', () => {
    expect(1).toBe(1);
});
console.log('hello');
"#,
    );
    let module = parse(&path);
    assert_eq!(module.test_blocks.len(), 1);
}

#[test]
fn mnt006_missing_await_assertion() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(
        &dir,
        "async_fail.test.ts",
        r#"
import { test, expect } from 'vitest';

test('missing await', async () => {
    expect(Promise.resolve(1)).resolves.toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-006");
    assert!(v.is_some(), "Expected VITEST-MNT-006 violation");
}

#[test]
fn val001_expect_without_assertion() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "bare_expect.test.ts",
        r#"
import { test, expect } from 'vitest';

test('bare expect', () => {
    expect(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-VAL-001");
    assert!(v.is_some(), "Expected VITEST-VAL-001 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "ValidExpectRule");
    assert_eq!(v.severity, vitest_linter::models::Severity::Error);
    assert_eq!(v.category, vitest_linter::models::Category::Validation);
}

#[test]
fn val001_no_violation_with_assertion_method() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "proper_expect.test.ts",
        r#"
import { test, expect } from 'vitest';

test('proper expect', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-VAL-001").is_none(),
        "Should not trigger VAL-001 when expect has assertion method"
    );
}

#[test]
fn val002_return_of_expect() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "return_expect.test.ts",
        r#"
import { test, expect } from 'vitest';

test('return expect', () => {
    return expect(Promise.resolve(1)).resolves.toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-VAL-002");
    assert!(v.is_some(), "Expected VITEST-VAL-002 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "ValidExpectInPromiseRule");
    assert_eq!(v.severity, vitest_linter::models::Severity::Error);
}

#[test]
fn val002_no_violation_with_await() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "await_expect.test.ts",
        r#"
import { test, expect } from 'vitest';

test('await expect', async () => {
    await expect(Promise.resolve(1)).resolves.toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-VAL-002").is_none(),
        "Should not trigger VAL-002 when using await"
    );
}

#[test]
fn val003_async_describe_callback() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "async_describe.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

describe('async describe', async () => {
    test('inside', () => {
        expect(1).toBe(1);
    });
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-VAL-003");
    assert!(v.is_some(), "Expected VITEST-VAL-003 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "ValidDescribeCallbackRule");
    assert_eq!(v.severity, vitest_linter::models::Severity::Error);
}

#[test]
fn val003_no_violation_sync_describe() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "sync_describe.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

describe('sync describe', () => {
    test('inside', () => {
        expect(1).toBe(1);
    });
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-VAL-003").is_none(),
        "Should not trigger VAL-003 for sync describe"
    );
}

#[test]
fn val004_template_literal_test_title() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "template_title.test.ts",
        r#"
import { test, expect } from 'vitest';

test(`template title`, () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-VAL-004");
    assert!(
        v.is_some(),
        "Expected VITEST-VAL-004 violation for template literal title"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "ValidTitleRule");
    assert_eq!(v.severity, vitest_linter::models::Severity::Warning);
}

#[test]
fn val004_empty_describe_title() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "empty_describe.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

describe('', () => {
    test('inside', () => {
        expect(1).toBe(1);
    });
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-VAL-004");
    assert!(
        v.is_some(),
        "Expected VITEST-VAL-004 violation for empty describe title"
    );
}

#[test]
fn val004_no_violation_with_string_titles() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "proper_titles.test.ts",
        r#"
import { describe, test, expect } from 'vitest';

describe('proper title', () => {
    test('proper test', () => {
        expect(1).toBe(1);
    });
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-VAL-004").is_none(),
        "Should not trigger VAL-004 for string titles"
    );
}

#[test]
fn val005_async_expect_wrapper() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "async_wrapper.test.ts",
        r#"
import { test, expect } from 'vitest';

test('async wrapper', () => {
    expect(async () => {
        await Promise.resolve(1);
    }).not.toThrow();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-VAL-005");
    assert!(v.is_some(), "Expected VITEST-VAL-005 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "NoUnneededAsyncExpectFunctionRule");
    assert_eq!(v.severity, vitest_linter::models::Severity::Warning);
}

#[test]
fn val005_no_violation_without_async_wrapper() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "sync_expect.test.ts",
        r#"
import { test, expect } from 'vitest';

test('sync expect', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-VAL-005").is_none(),
        "Should not trigger VAL-005 for sync expect"
    );
}

// ===========================================================================
// E12: No-rules integration tests
// ===========================================================================

// --- VITEST-NO-001: NoStandaloneExpectRule ---

#[test]
fn no001_standalone_expect_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "standalone.test.ts",
        r#"
import { expect } from 'vitest';

expect(true).toBe(true);

test('inside test', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-NO-001");
    assert!(v.is_some(), "Expected VITEST-NO-001 violation");
    assert_eq!(v.unwrap().rule_name, "NoStandaloneExpectRule");
}

#[test]
fn no001_no_violation_when_expect_inside_test() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "inside.test.ts",
        r#"
import { test, expect } from 'vitest';

test('ok', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-NO-001").is_none(),
        "Should not trigger NO-001 when expect is inside test"
    );
}

// --- VITEST-NO-002: NoIdenticalTitleRule ---

#[test]
fn no002_duplicate_test_title_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "dup_title.test.ts",
        r#"
import { test, expect } from 'vitest';

test('same title', () => { expect(1).toBe(1); });
test('same title', () => { expect(2).toBe(2); });
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-NO-002");
    assert!(v.is_some(), "Expected VITEST-NO-002 violation");
    assert_eq!(v.unwrap().rule_name, "NoIdenticalTitleRule");
}

#[test]
fn no002_unique_titles_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "unique.test.ts",
        r#"
import { test, expect } from 'vitest';

test('first', () => { expect(1).toBe(1); });
test('second', () => { expect(2).toBe(2); });
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-NO-002").is_none(),
        "Should not trigger NO-002 for unique titles"
    );
}

// --- VITEST-NO-003: NoCommentedOutTestsRule ---

#[test]
fn no003_commented_test_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "commented.test.ts",
        r#"
import { test, expect } from 'vitest';

// test('disabled test', () => {
//     expect(1).toBe(1);
// });

test('active', () => { expect(1).toBe(1); });
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-NO-003");
    assert!(v.is_some(), "Expected VITEST-NO-003 violation");
    assert_eq!(v.unwrap().rule_name, "NoCommentedOutTestsRule");
}

#[test]
fn no003_no_comments_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "nocomment.test.ts",
        r#"
import { test, expect } from 'vitest';

test('active', () => { expect(1).toBe(1); });
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-NO-003").is_none(),
        "Should not trigger NO-003 without commented tests"
    );
}

#[test]
fn no003_commented_test_no_space_after_marker_triggers() {
    // A commented-out test with no space after `//` (e.g. `//test(`) must
    // still be detected. Guards against the prior false negative where only
    // `// test(` (with a space) matched the pattern set.
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "commented_no_space.test.ts",
        r#"
import { test, expect } from 'vitest';

//test('disabled', () => {
//    expect(1).toBe(1);
//});

test('active', () => { expect(1).toBe(1); });
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-NO-003");
    assert!(
        v.is_some(),
        "Expected VITEST-NO-003 for `//test(` even without a space after //"
    );
}

// --- VITEST-NO-005: NoTestPrefixesRule ---

#[test]
fn no005_fit_prefix_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "fit.test.ts",
        r#"
import { expect } from 'vitest';

fit('focused test', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-NO-005");
    assert!(v.is_some(), "Expected VITEST-NO-005 violation");
    assert_eq!(v.unwrap().rule_name, "NoTestPrefixesRule");
}

#[test]
fn no005_no_prefix_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "noprefix.test.ts",
        r#"
import { test, expect } from 'vitest';

test('normal test', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-NO-005").is_none(),
        "Should not trigger NO-005 for normal test()"
    );
}

// --- VITEST-NO-006: NoDuplicateHooksRule ---

#[test]
fn no006_duplicate_before_each_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "duphooks.test.ts",
        r#"
import { beforeEach, test, expect } from 'vitest';

beforeEach(() => { /* setup 1 */ });
beforeEach(() => { /* setup 2 */ });

test('ok', () => { expect(1).toBe(1); });
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-NO-006");
    assert!(v.is_some(), "Expected VITEST-NO-006 violation");
    assert_eq!(v.unwrap().rule_name, "NoDuplicateHooksRule");
}

#[test]
fn no006_single_hooks_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "singlehooks.test.ts",
        r#"
import { beforeEach, afterEach, test, expect } from 'vitest';

beforeEach(() => { /* setup */ });
afterEach(() => { /* teardown */ });

test('ok', () => { expect(1).toBe(1); });
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-NO-006").is_none(),
        "Should not trigger NO-006 for single hooks"
    );
}

// --- VITEST-NO-007: NoImportNodeTestRule ---

#[test]
fn no007_import_node_test_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "nodetest.test.ts",
        r#"
import { test, expect } from 'node:test';

test('bad import', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-NO-007");
    assert!(v.is_some(), "Expected VITEST-NO-007 violation");
    assert_eq!(v.unwrap().rule_name, "NoImportNodeTestRule");
}

#[test]
fn no007_import_vitest_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "vitest.test.ts",
        r#"
import { test, expect } from 'vitest';

test('ok', () => { expect(1).toBe(1); });
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-NO-007").is_none(),
        "Should not trigger NO-007 for vitest import"
    );
}

// --- VITEST-NO-008: NoInterpolationInSnapshotsRule ---

#[test]
fn no008_template_in_snapshot_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "interpsnap.test.ts",
        r#"
import { test, expect } from 'vitest';

test('snapshot with interpolation', () => {
    const name = 'world';
    expect(`hello ${name}`).toMatchInlineSnapshot(`"hello ${name}"`);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-NO-008");
    assert!(v.is_some(), "Expected VITEST-NO-008 violation");
    assert_eq!(v.unwrap().rule_name, "NoInterpolationInSnapshotsRule");
}

#[test]
fn no008_static_snapshot_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "staticsnap.test.ts",
        r#"
import { test, expect } from 'vitest';

test('static snapshot', () => {
    expect('hello').toMatchInlineSnapshot('"hello"');
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-NO-008").is_none(),
        "Should not trigger NO-008 for static snapshot"
    );
}

// --- VITEST-NO-009: NoLargeSnapshotsRule ---

#[test]
fn no009_large_snapshot_triggers() {
    let dir = TempDir::new().unwrap();
    // Generate a snapshot string that's > 50 lines (valid JS template literal)
    let mut snapshot = String::from("`\n");
    for i in 0..55 {
        snapshot.push_str(&format!("    line {}\n", i));
    }
    snapshot.push('`');
    let content = format!(
        r#"
import {{ test, expect }} from 'vitest';

test('large snapshot', () => {{
    expect('data').toMatchInlineSnapshot({});
}});
"#,
        snapshot
    );
    let path = write_fixture(&dir, "largesnap.test.ts", &content);
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-NO-009");
    assert!(v.is_some(), "Expected VITEST-NO-009 violation");
    assert_eq!(v.unwrap().rule_name, "NoLargeSnapshotsRule");
}

#[test]
fn no009_small_snapshot_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "smallsnap.test.ts",
        r#"
import { test, expect } from 'vitest';

test('small snapshot', () => {
    expect('hello').toMatchInlineSnapshot('"hello"');
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-NO-009").is_none(),
        "Should not trigger NO-009 for small snapshot"
    );
}

// --- VITEST-NO-013: NoDoneCallbackRule ---

#[test]
fn no013_done_callback_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "done.test.ts",
        r#"
import { test, expect } from 'vitest';

test('uses done', (done) => {
    setTimeout(() => {
        expect(true).toBe(true);
        done();
    }, 10);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-NO-013");
    assert!(v.is_some(), "Expected VITEST-NO-013 violation");
    assert_eq!(v.unwrap().rule_name, "NoDoneCallbackRule");
}

#[test]
fn no013_async_await_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "async.test.ts",
        r#"
import { test, expect } from 'vitest';

test('uses async/await', async () => {
    const result = await Promise.resolve(1);
    expect(result).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-NO-013").is_none(),
        "Should not trigger NO-013 for async/await"
    );
}

// --- VITEST-NO-014: NoConditionalExpectRule ---

#[test]
fn no014_conditional_expect_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "condexpect.test.ts",
        r#"
import { test, expect } from 'vitest';

test('conditional expect', () => {
    const value = true;
    if (value) {
        expect(value).toBe(true);
    }
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-NO-014");
    assert!(v.is_some(), "Expected VITEST-NO-014 violation");
    assert_eq!(v.unwrap().rule_name, "NoConditionalExpectRule");
}

#[test]
fn no014_unconditional_expect_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "unconditional.test.ts",
        r#"
import { test, expect } from 'vitest';

test('unconditional expect', () => {
    expect(1 + 1).toBe(2);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-NO-014").is_none(),
        "Should not trigger NO-014 for unconditional expect"
    );
}

// --- VITEST-PREF-001: PreferToBeRule ---

#[test]
fn pref001_to_equal_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "tobe.test.ts",
        r#"
import { test, expect } from 'vitest';

test('use toBe', () => {
    expect(true).toEqual(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-PREF-001");
    assert!(v.is_some(), "Expected VITEST-PREF-001 violation");
    assert_eq!(v.unwrap().rule_name, "PreferToBeRule");
}

#[test]
fn pref001_to_be_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "tobe_ok.test.ts",
        r#"
import { test, expect } from 'vitest';

test('use toBe', () => {
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-PREF-001").is_none(),
        "Should not trigger PREF-001 for toBe"
    );
}

#[test]
fn pref001_no_false_positive_on_keyword_prefix_identifier() {
    // Identifiers that merely start with a keyword (trueValue, nullish,
    // 123abc) must NOT be mistaken for primitive literals. Guards against
    // the prior substring/prefix false positive.
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "tobe_falsepos.test.ts",
        r#"
import { test, expect } from 'vitest';

test('keyword-prefix identifiers', () => {
    const trueValue = { a: 1 };
    expect(x).toEqual(trueValue);
    expect(y).toEqual(nullish);
    expect(z).toEqual(123abc);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-PREF-001").is_none(),
        "Identifiers like trueValue/nullish/123abc must NOT trigger PREF-001"
    );
}

// --- VITEST-PREF-002: PreferToContainRule ---

#[test]
fn pref002_length_gt_zero_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "tocontain.test.ts",
        r#"
import { test, expect } from 'vitest';

test('use toContain', () => {
    expect([1, 2, 3].includes(2)).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-PREF-002");
    assert!(v.is_some(), "Expected VITEST-PREF-002 violation");
}

#[test]
fn pref002_to_contain_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "tocontain_ok.test.ts",
        r#"
import { test, expect } from 'vitest';

test('use toContain', () => {
    expect([1, 2, 3]).toContain(2);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-PREF-002").is_none(),
        "Should not trigger PREF-002 for toContain"
    );
}

// --- VITEST-PREF-003: PreferToHaveLengthRule ---

#[test]
fn pref003_length_be_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "tohavelength.test.ts",
        r#"
import { test, expect } from 'vitest';

test('use toHaveLength', () => {
    expect(arr.length).toBe(3);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-PREF-003");
    assert!(v.is_some(), "Expected VITEST-PREF-003 violation");
}

#[test]
fn pref003_to_have_length_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "tohavelength_ok.test.ts",
        r#"
import { test, expect } from 'vitest';

test('use toHaveLength', () => {
    expect(arr).toHaveLength(3);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-PREF-003").is_none(),
        "Should not trigger PREF-003 for toHaveLength"
    );
}

// --- VITEST-PREF-005: PreferSpyOnRule ---

#[test]
fn pref005_jest_fn_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "spyon.test.ts",
        r#"
import { test, vi } from 'vitest';

test('use spyOn', () => {
    const mock = vi.fn();
    obj.method = mock;
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-PREF-005");
    assert!(v.is_some(), "Expected VITEST-PREF-005 violation");
}

#[test]
fn pref005_spy_on_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "spyon_ok.test.ts",
        r#"
import { test, vi } from 'vitest';

test('use spyOn', () => {
    vi.spyOn(obj, 'method');
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-PREF-005").is_none(),
        "Should not trigger PREF-005 for spyOn"
    );
}

// --- VITEST-PREF-007: PreferCalledOnceRule ---

#[test]
fn pref007_to_have_been_called_times_1_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "calledonce.test.ts",
        r#"
import { test, expect, vi } from 'vitest';

test('use toHaveBeenCalledOnce', () => {
    expect(mockFn).toHaveBeenCalledTimes(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-PREF-007");
    assert!(v.is_some(), "Expected VITEST-PREF-007 violation");
}

#[test]
fn pref007_to_have_been_called_once_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "calledonce_ok.test.ts",
        r#"
import { test, expect, vi } from 'vitest';

test('use toHaveBeenCalledOnce', () => {
    expect(mockFn).toHaveBeenCalledOnce();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-PREF-007").is_none(),
        "Should not trigger PREF-007 for toHaveBeenCalledOnce"
    );
}

// --- VITEST-PREF-009: PreferHooksOnTopRule ---

#[test]
fn pref009_hook_after_test_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "hooksontop.test.ts",
        r#"
import { test, vi, beforeEach } from 'vitest';

test('a', () => {});
beforeEach(() => {});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-PREF-009");
    assert!(v.is_some(), "Expected VITEST-PREF-009 violation");
}

// --- VITEST-PREF-010: PreferHooksInOrderRule ---

#[test]
fn pref010_wrong_hook_order_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "hooksinorder.test.ts",
        r#"
import { beforeEach, beforeAll } from 'vitest';

beforeEach(() => {});
beforeAll(() => {});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-PREF-010");
    assert!(v.is_some(), "Expected VITEST-PREF-010 violation");
}

// --- VITEST-PREF-012: PreferTodoRule ---

#[test]
fn pref012_empty_test_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "todo.test.ts",
        r#"
import { test } from 'vitest';

test('todo this', () => {});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-PREF-012");
    assert!(v.is_some(), "Expected VITEST-PREF-012 violation");
}

// --- VITEST-PREF-013: PreferMockPromiseShorthandRule ---

#[test]
fn pref013_mock_implementation_with_resolve_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "mockshorthand.test.ts",
        r#"
import { vi } from 'vitest';

vi.fn().mockImplementation(() => Promise.resolve(42));
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-PREF-013");
    assert!(v.is_some(), "Expected VITEST-PREF-013 violation");
}

// --- VITEST-PREF-014: PreferExpectResolvesRule ---

#[test]
fn pref014_expect_await_rejects_to_be_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "expectresolves.test.ts",
        r#"
import { test, expect } from 'vitest';

test('use resolves', async () => {
    expect(await Promise.resolve(1)).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-PREF-014");
    assert!(v.is_some(), "Expected VITEST-PREF-014 violation");
}

// --- VITEST-REQ-001: RequireHookRule ---

#[test]
fn req001_setup_outside_hook_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "requirehook.test.ts",
        r#"
import { test, vi } from 'vitest';

vi.mock('./foo');
test('a', () => {});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-REQ-001");
    assert!(v.is_some(), "Expected VITEST-REQ-001 violation");
}

// --- VITEST-REQ-002: RequireTopLevelDescribeRule ---

#[test]
fn req002_orphan_test_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "toplevel.test.ts",
        r#"
import { test, describe } from 'vitest';

describe('group', () => {
    test('inside', () => {});
});

test('orphan', () => {});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-REQ-002");
    assert!(v.is_some(), "Expected VITEST-REQ-002 violation");
}

// --- VITEST-REQ-003: RequireToThrowMessageRule ---

#[test]
fn req003_to_throw_no_message_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "tothrowmsg.test.ts",
        r#"
import { test, expect } from 'vitest';

test('require message', () => {
    expect(() => { throw new Error('fail'); }).toThrow();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-REQ-003");
    assert!(v.is_some(), "Expected VITEST-REQ-003 violation");
}

// --- VITEST-CON-001: ConsistentTestItRule ---

#[test]
fn con001_mixed_test_it_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "consistent.test.ts",
        r#"
import { test, it } from 'vitest';

test('a', () => {});
it('b', () => {});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-CON-001");
    assert!(v.is_some(), "Expected VITEST-CON-001 violation");
}

#[test]
fn con001_test_describe_with_it_triggers() {
    // `test.describe(` is one of the `test.*` forms that should set has_test.
    // Combined with `it(`, this must still trigger CON-001. Guards the `||`
    // chain in ConsistentTestItRule (a `&&` mutant would drop test.describe).
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "consistent_describe.test.ts",
        r#"
import { test, it } from 'vitest';

test.describe('group', () => {
    it('b', () => {});
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-CON-001");
    assert!(
        v.is_some(),
        "Expected VITEST-CON-001 violation when mixing test.describe and it"
    );
}

#[test]
fn con001_each_test_form_with_it_triggers() {
    // Every `test.*` form must set has_test=true, so that mixing it with
    // `it(` triggers CON-001. This guards each `||` in the has_test chain
    // against `&&` mutants, which would otherwise drop a single form.
    let forms = [
        "test.skip(",
        "test.only(",
        "test.todo(",
        "test.concurrent(",
        "test.each([])(",
        "test.describe(",
    ];
    for form in forms {
        let dir = TempDir::new().unwrap();
        let body = format!(
            "import {{ test, it }} from 'vitest';\n\n{form}('a', () => {{}});\nit('b', () => {{}});\n"
        );
        let path = dir.path().join("consistent_form.test.ts");
        std::fs::write(&path, body).unwrap();
        let engine = LintEngine::new(true).unwrap();
        let violations = engine.lint_paths(&[path]).unwrap();
        assert!(
            find_violation(&violations, "VITEST-CON-001").is_some(),
            "Expected VITEST-CON-001 when mixing {form} and it()"
        );
    }
}

#[test]
fn con001_each_it_form_with_test_triggers() {
    // Symmetric to the test.* coverage: every `it.*` form must set has_it=true,
    // so that mixing it with `test(` triggers CON-001. Guards each `||` in the
    // has_it chain against `&&` mutants.
    let forms = [
        "it.skip(",
        "it.only(",
        "it.todo(",
        "it.concurrent(",
        "it.each([])(",
    ];
    for form in forms {
        let dir = TempDir::new().unwrap();
        let body = format!(
            "import {{ test, it }} from 'vitest';\n\ntest('a', () => {{}});\n{form}('b', () => {{}});\n"
        );
        let path = dir.path().join("consistent_it_form.test.ts");
        std::fs::write(&path, body).unwrap();
        let engine = LintEngine::new(true).unwrap();
        let violations = engine.lint_paths(&[path]).unwrap();
        assert!(
            find_violation(&violations, "VITEST-CON-001").is_some(),
            "Expected VITEST-CON-001 when mixing test() and {form}"
        );
    }
}

// --- VITEST-CON-003: ConsistentVitestViRule ---

#[test]
fn con003_vi_and_vitest_namespace_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "mixed_vi.test.ts",
        r#"
import { vi } from 'vitest';
import * as vitest from 'vitest';
import { test } from 'vitest';

test('a', () => {});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-CON-003");
    assert!(
        v.is_some(),
        "Expected VITEST-CON-003 violation for vi + namespace mix"
    );
}

#[test]
fn con003_no_false_positive_on_substring_binding() {
    // A file importing a binding whose name merely contains "vi" as a
    // substring (e.g. `visit`) must NOT be treated as importing the `vi`
    // mock helper. Combined with a namespace import this previously caused
    // a false CON-003 violation.
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "visit.test.ts",
        r#"
import * as vitest from 'vitest';
import { visit, describe, expect } from 'vitest';

describe('suite', () => {
    visit();
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-CON-003");
    assert!(
        v.is_none(),
        "importing `visit` (substring of vi) must NOT trigger VITEST-CON-003"
    );
}

// --- VITEST-CON-004: HoistedApisOnTopRule ---

#[test]
fn con004_mock_after_test_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "hoisted.test.ts",
        r#"
import { test, vi } from 'vitest';

test('a', () => {});
vi.mock('./foo');
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-CON-004");
    assert!(v.is_some(), "Expected VITEST-CON-004 violation");
}

// --- VITEST-FLK-004: Fake Timer Cleanup with inline vi.useRealTimers() ---

#[test]
fn flk004_fake_timers_with_inline_cleanup() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "fake_timers_inline_cleanup.test.ts",
        r#"
import { test, expect, vi } from 'vitest';

test('uses fake timers with inline cleanup', () => {
    vi.useFakeTimers();
    expect(true).toBe(true);
    vi.useRealTimers();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-FLK-004").is_none(),
        "Should not trigger FLK-004 when test calls vi.useRealTimers() inline"
    );
}

#[test]
fn flk004_fake_timers_no_cleanup_still_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "fake_timers_no_cleanup2.test.ts",
        r#"
import { test, expect, vi } from 'vitest';

test('uses fake timers without cleanup', () => {
    vi.useFakeTimers();
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-FLK-004").is_some(),
        "Should trigger FLK-004 when no cleanup exists"
    );
}

// --- Export parsing ---

#[test]
fn parser_collects_exports() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "module.ts",
        r#"
export function foo() { return 1; }
export const bar = 2;
export default class Baz {}
"#,
    );
    let module = parse(&path);
    assert!(module.exports.len() >= 2, "Expected at least 2 exports");
    assert!(module.exports.iter().any(|e| e.name == "foo"));
    assert!(module.exports.iter().any(|e| e.name == "bar"));
}

#[test]
fn parser_collects_reexports() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "reexport.ts",
        r#"
export { default as Foo } from './foo';
export { bar, baz } from './bar';
"#,
    );
    let module = parse(&path);
    assert!(module.exports.len() >= 2, "Expected at least 2 re-exports");
}

// --- VITEST-DEP-004: Mock Export Validation ---

#[test]
fn dep004_mock_with_matching_factory_keys() {
    let dir = TempDir::new().unwrap();
    let source_path = write_fixture(
        &dir,
        "my-module.ts",
        r#"
export function foo() { return 1; }
export function bar() { return 2; }
"#,
    );
    let test_path = write_fixture(
        &dir,
        "my-module.test.ts",
        r#"
import { test, expect, vi } from 'vitest';

vi.mock('./my-module', () => ({
    foo: vi.fn(),
    bar: vi.fn(),
}));

test('mocks', () => {
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[test_path, source_path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-DEP-004").is_none(),
        "Should not trigger DEP-004 when factory keys match exports"
    );
}

#[test]
fn dep004_mock_with_extra_factory_key() {
    let dir = TempDir::new().unwrap();
    let source_path = write_fixture(
        &dir,
        "my-module2.ts",
        r#"
export function foo() { return 1; }
export function bar() { return 2; }
"#,
    );
    let test_path = write_fixture(
        &dir,
        "my-module2.test.ts",
        r#"
import { test, expect, vi } from 'vitest';

vi.mock('./my-module2', () => ({
    foo: vi.fn(),
    nonexistent: vi.fn(),
}));

test('mocks', () => {
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[test_path, source_path]).unwrap();
    let v = find_violation(&violations, "VITEST-DEP-004");
    assert!(
        v.is_some(),
        "Expected VITEST-DEP-004 for extra factory key 'nonexistent'"
    );
}

#[test]
fn dep004_mock_with_fewer_keys_no_violation() {
    let dir = TempDir::new().unwrap();
    let source_path = write_fixture(
        &dir,
        "my-module3.ts",
        r#"
export function foo() { return 1; }
export function bar() { return 2; }
export function baz() { return 3; }
"#,
    );
    let test_path = write_fixture(
        &dir,
        "my-module3.test.ts",
        r#"
import { test, expect, vi } from 'vitest';

vi.mock('./my-module3', () => ({
    foo: vi.fn(),
}));

test('mocks', () => {
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[test_path, source_path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-DEP-004").is_none(),
        "Should not trigger DEP-004 when mock has fewer keys than source (missing keys are not extra)"
    );
}

// ===========================================================================
// E13: WeakAssertionRule (MNT-009) integration tests
// ===========================================================================

#[test]
fn mnt009_all_weak_assertions_defined() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "weak_defined.test.ts",
        r#"
import { test, expect } from 'vitest';

test('all weak', () => {
    expect(result).toBeDefined();
    expect(value).toBeUndefined();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-009");
    assert!(
        v.is_some(),
        "Expected VITEST-MNT-009 for all weak assertions"
    );
    assert_eq!(v.unwrap().rule_name, "WeakAssertionRule");
}

#[test]
fn mnt009_truthy_falsy_weak() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "weak_truthy.test.ts",
        r#"
import { test, expect } from 'vitest';

test('truthy falsy', () => {
    expect(flag).toBeTruthy();
    expect(other).toBeFalsy();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-009");
    assert!(
        v.is_some(),
        "Expected VITEST-MNT-009 for toBeTruthy/toBeFalsy"
    );
}

#[test]
fn mnt009_not_to_throw_weak() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "weak_not_throw.test.ts",
        r#"
import { test, expect } from 'vitest';

test('not toThrow', () => {
    expect(() => doSomething()).not.toThrow();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-009");
    assert!(v.is_some(), "Expected VITEST-MNT-009 for not.toThrow()");
}

#[test]
fn mnt009_mixed_weak_and_strong_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "mixed_assertions.test.ts",
        r#"
import { test, expect } from 'vitest';

test('mixed', () => {
    expect(result).toBeDefined();
    expect(result.name).toBe('test');
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-MNT-009").is_none(),
        "Should not trigger MNT-009 when strong assertions exist"
    );
}

#[test]
fn mnt009_strong_assertions_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "strong_assertions.test.ts",
        r#"
import { test, expect } from 'vitest';

test('strong', () => {
    expect(value).toBe(42);
    expect(list).toContain('item');
    expect(num).toBeGreaterThan(0);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-MNT-009").is_none(),
        "Should not trigger MNT-009 for strong assertions"
    );
}

#[test]
fn mnt009_no_assertions_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "no_assertions_mnt009.test.ts",
        r#"
import { test } from 'vitest';

test('no assertions', () => {
    const x = 1;
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-MNT-009").is_none(),
        "Should not trigger MNT-009 when no assertions exist (MNT-001 handles that)"
    );
}

// --- VITEST-DEP-001: Banned Module Mock ---
#[test]
fn dep001_banned_mock_path_triggers() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join(".vitest-linter.toml"),
        r#"
[deps]
banned_mock_paths = ["**/database"]
"#,
    )
    .unwrap();
    let path = write_fixture(
        &dir,
        "dep001_banned.test.ts",
        r#"
import { vi, test, expect } from 'vitest';
vi.mock('./database', () => ({ db: {} }));

test('works', () => {
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-DEP-001");
    assert!(v.is_some(), "Expected VITEST-DEP-001 for banned mock path");
    assert!(v.unwrap().message.contains("database"));
}

#[test]
fn dep001_non_banned_mock_no_violation() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join(".vitest-linter.toml"),
        r#"
[deps]
banned_mock_paths = ["**/database"]
"#,
    )
    .unwrap();
    let path = write_fixture(
        &dir,
        "dep001_ok.test.ts",
        r#"
import { vi, test, expect } from 'vitest';
vi.mock('./api', () => ({ helper: () => {} }));

test('works', () => {
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-DEP-001").is_none(),
        "Should not trigger DEP-001 for non-banned path"
    );
}

// --- VITEST-DEP-002: Production Singleton Import ---
#[test]
fn dep002_banned_singleton_import_triggers() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join(".vitest-linter.toml"),
        r#"
[[deps.banned_singletons]]
from = "**/db"
names = ["db"]
"#,
    )
    .unwrap();
    let path = write_fixture(
        &dir,
        "dep002_singleton.test.ts",
        r#"
import { test, expect } from 'vitest';
import { db } from './db';

test('works', () => {
    expect(db).toBeDefined();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-DEP-002");
    assert!(
        v.is_some(),
        "Expected VITEST-DEP-002 for banned singleton import"
    );
    assert!(v.unwrap().message.contains("db"));
}

#[test]
fn dep002_integration_test_glob_skips() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join(".vitest-linter.toml"),
        r#"
[deps]
integration_test_glob = "**/*.integration.test.ts"

[[deps.banned_singletons]]
from = "**/db"
names = ["db"]
"#,
    )
    .unwrap();
    let path = write_fixture(
        &dir,
        "dep002.integration.test.ts",
        r#"
import { test, expect } from 'vitest';
import { db } from './db';

test('works', () => {
    expect(db).toBeDefined();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-DEP-002").is_none(),
        "Should not trigger DEP-002 for integration test files"
    );
}

// --- VITEST-DEP-003: Reset Escape Hatch ---
#[test]
fn dep003_restore_all_mocks_in_after_each_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "dep003_escape.test.ts",
        r#"
import { afterEach, vi, test, expect } from 'vitest';

afterEach(() => {
    vi.restoreAllMocks();
});

test('works', () => {
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-DEP-003");
    assert!(
        v.is_some(),
        "Expected VITEST-DEP-003 for vi.restoreAllMocks in afterEach"
    );
    assert!(v.unwrap().message.contains("vi.restoreAllMocks"));
}

#[test]
fn dep003_no_escape_hatch_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "dep003_ok.test.ts",
        r#"
import { afterEach, vi, test, expect } from 'vitest';

afterEach(() => {
    vi.clearAllMocks();
});

test('works', () => {
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-DEP-003").is_none(),
        "Should not trigger DEP-003 for vi.clearAllMocks (not an escape hatch)"
    );
}

// --- VITEST-FLK-005: Non-Deterministic ---
#[test]
fn flk005_math_random_triggers() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "flk005_random.test.ts",
        r#"
import { test, expect } from 'vitest';

test('random value', () => {
    const val = Math.random();
    expect(val).toBeGreaterThanOrEqual(0);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-FLK-005");
    assert!(
        v.is_some(),
        "Expected VITEST-FLK-005 for Math.random() in test"
    );
    assert!(v.unwrap().message.contains("Math.random"));
}

#[test]
fn flk005_no_random_no_violation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "flk005_ok.test.ts",
        r#"
import { test, expect } from 'vitest';

test('deterministic', () => {
    const val = 42;
    expect(val).toBe(42);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-FLK-005").is_none(),
        "Should not trigger FLK-005 without Math.random()"
    );
}

// --- VITEST-MNT-010: Implementation Coupled ---
#[test]
fn mnt010_implementation_coupled_triggers() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("counter.ts"),
        r#"
export function add(a: number, b: number): number { return a + b; }
export function subtract(a: number, b: number): number { return a - b; }
"#,
    )
    .unwrap();
    let path = write_fixture(
        &dir,
        "counter.test.ts",
        r#"
import { test, expect } from 'vitest';
import { add, subtract } from './counter';

test('add', () => { expect(add(1, 2)).toBe(3); });
test('subtract', () => { expect(subtract(5, 3)).toBe(2); });
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-MNT-010").is_some(),
        "MNT-010 should trigger: single production import, test names match export names"
    );
}

#[test]
fn mnt007_playwright_test_only() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "pw-only.spec.ts",
        r#"
import { test, expect } from '@playwright/test';

test.only('focused pw test', async ({ page }) => {
    await expect(page).toHaveTitle(/app/);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-007");
    assert!(
        v.is_some(),
        "Expected VITEST-MNT-007 for Playwright test.only"
    );
    assert_eq!(v.unwrap().rule_name, "FocusedTestRule");
}

#[test]
fn mnt007_playwright_describe_only() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "pw-describe-only.spec.ts",
        r#"
import { test, expect } from '@playwright/test';

test.describe.only('focused group', () => {
    test('inside', async ({ page }) => {
        await expect(page).toHaveTitle(/app/);
    });
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-007");
    assert!(
        v.is_some(),
        "Expected VITEST-MNT-007 for Playwright describe.only"
    );
}

#[test]
fn vitest_rules_do_not_fire_on_playwright_files() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "pw-clean.spec.ts",
        r#"
import { test, expect } from '@playwright/test';
import axios from 'axios';

test('pw test with vitest-only triggers', async ({ page }) => {
    setTimeout(() => {}, 100);
    await expect(page).toHaveTitle(/app/);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    // These rules use the default `applies_to_runtime` (Vitest-only) and so
    // must NOT fire on a Playwright file, even when their trigger patterns
    // are present. This guards the default `runtime != Playwright` guard.
    let vitest_only_rule_ids = [
        "VITEST-FLK-001",
        "VITEST-FLK-002",
        "VITEST-FLK-003",
        "VITEST-MNT-002",
        "VITEST-MNT-003",
        "VITEST-MNT-004",
        "VITEST-MNT-008",
        "VITEST-MNT-009",
        "VITEST-DEP-001",
        "VITEST-PREF-003",
    ];
    for rule_id in &vitest_only_rule_ids {
        assert!(
            find_violation(&violations, rule_id).is_none(),
            "Vitest-only rule {} should not fire on Playwright file",
            rule_id
        );
    }
}

#[test]
fn cross_runtime_rules_fire_on_playwright() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "pw-no-assert.spec.ts",
        r#"
import { test } from '@playwright/test';

test('no assertions pw', async ({ page }) => {
    await page.goto('/');
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-001");
    assert!(
        v.is_some(),
        "NoAssertionRule should fire on Playwright files too"
    );
}

#[test]
fn mnt008_global_stub_without_cleanup() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "global_stub.test.ts",
        r#"
import { vi, test, expect } from 'vitest';

const mockFetch = vi.fn();
global.fetch = mockFetch;

test('uses global fetch', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-008");
    assert!(
        v.is_some(),
        "Expected VITEST-MNT-008 for global.fetch stub without cleanup"
    );
}

#[test]
fn mnt008_vi_stub_global_without_unstub() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "stub_global.test.ts",
        r#"
import { vi, test, expect } from 'vitest';

vi.stubGlobal('fetch', vi.fn(() => Promise.resolve({ ok: true })));

test('uses stubbed fetch', () => {
    expect(1).toBe(1);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-008");
    assert!(
        v.is_some(),
        "Expected VITEST-MNT-008 for vi.stubGlobal without vi.unstubAllGlobals"
    );
}

#[test]
fn pw003_flags_xpath() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "xpath.spec.ts",
        r#"
import { test, expect } from '@playwright/test';

test('xpath locator', async ({ page }) => {
    const el = page.locator('//div[@id="app"]');
    await expect(el).toBeVisible();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-PW-003").is_some(),
        "PW-003 should flag xpath= prefix in locator"
    );
}

#[test]
fn pw002_flags_css_id() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "pw-css-id.spec.ts",
        r#"
import { test } from '@playwright/test';

test('id selector', async ({ page }) => {
    const el = page.locator('#login-button');
    await expect(el).toBeVisible();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-PW-002").is_some(),
        "PW-002 should flag CSS ID selector"
    );
}

#[test]
fn pw006_flags_evaluate_inner_text() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "pw-evaluate.spec.ts",
        r#"
import { test } from '@playwright/test';

test('evaluate inner text', async ({ page }) => {
    const text = await page.evaluate(() => document.body.innerText);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-PW-006").is_some(),
        "PW-006 should flag page.evaluate with innerText"
    );
}

#[test]
fn pw011_flags_hard_css_chain() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "pw-css-chain.spec.ts",
        r#"
import { test } from '@playwright/test';

test('chained css selectors', async ({ page }) => {
    const el = page.locator('.foo > .bar');
    await expect(el).toBeVisible();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        find_violation(&violations, "VITEST-PW-011").is_some(),
        "PW-011 should flag hard CSS class chain"
    );
}

#[test]
fn mnt007_playwright_fixture_violates() {
    let fixture_dir =
        std::path::Path::new("tests/fixtures/mnt_007_focused_test/playwright_test_only");
    assert!(fixture_dir.exists(), "Fixture directory should exist");
    let input_path = fixture_dir.join("input.spec.ts");
    assert!(input_path.exists(), "Fixture input file should exist");

    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[input_path]).unwrap();

    let mnt007: Vec<_> = violations
        .iter()
        .filter(|v| v.rule_id == "VITEST-MNT-007")
        .collect();

    assert_eq!(mnt007.len(), 2, "Expected 2 FocusedTestRule violations");

    let describe_v: Vec<_> = mnt007.iter().filter(|v| v.test_name.is_none()).collect();
    let test_v: Vec<_> = mnt007.iter().filter(|v| v.test_name.is_some()).collect();

    assert_eq!(describe_v.len(), 1, "Expected 1 describe.only violation");
    assert_eq!(test_v.len(), 1, "Expected 1 test.only violation");

    assert_eq!(describe_v[0].line, 3);
    assert_eq!(test_v[0].line, 9);
    assert_eq!(
        test_v[0].test_name.as_deref(),
        Some("standalone focused test"),
    );
}

#[test]
fn flk001_flags_promise_wrapped_settimeout() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "promise_settimeout.test.ts",
        r#"
import { vi, test, expect } from 'vitest';

test('waits with promise setTimeout', async () => {
    await new Promise(resolve => setTimeout(resolve, 1000));
    expect(true).toBe(true);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-FLK-001");
    assert!(
        v.is_some(),
        "FLK-001 should flag Promise-wrapped setTimeout"
    );
    if let Some(viol) = v {
        assert!(
            viol.message.contains("Promise"),
            "FLK-001 message should mention Promise for wrapped setTimeout"
        );
    }
}

#[test]
fn mnt011_flags_testid_without_negative_presence() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "testid_no_negative.spec.ts",
        r#"
import { test, expect } from '@playwright/test';

test('finds element by testid', async ({ page }) => {
    const el = page.getByTestId('submit-btn');
    await expect(el).toBeVisible();
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-MNT-011");
    assert!(
        v.is_some(),
        "MNT-011 should flag getByTestId without negative-presence assertion"
    );
}

#[test]
fn pref003_flags_size_on_map_set() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(
        &dir,
        "size_assertion.test.ts",
        r#"
import { test, expect } from 'vitest';

test('map size', () => {
    const m = new Map();
    expect(m.size).toBe(0);
});
"#,
    );
    let engine = LintEngine::new(true).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "VITEST-PREF-003");
    assert!(
        v.is_some(),
        "PREF-003 should flag .size assertion on Map/Set"
    );
}
