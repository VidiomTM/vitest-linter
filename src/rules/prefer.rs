use crate::models::{Category, HookKind, ModuleGraph, ParsedModule, Severity, Violation};
use crate::rules::Rule;

/// True when `arg` is exactly `keyword` (possibly followed by `)`, `,`, or
/// trailing whitespace), so that identifiers like `trueValue` or `nullish`
/// are NOT mistaken for the literal keyword `true`/`null`.
fn is_exact_keyword(arg: &str, keyword: &str) -> bool {
    if let Some(rest) = arg.strip_prefix(keyword) {
        rest.is_empty()
            || rest.starts_with(')')
            || rest.starts_with(',')
            || rest.starts_with(';')
            || rest.starts_with(' ')
            || rest.starts_with('\t')
            || rest.starts_with("//")
    } else {
        false
    }
}

/// True when `arg` is a single numeric literal (e.g. `123`, `3.14`, `-1`),
/// so that identifiers starting with a digit like `123abc` are NOT matched.
/// Trailing `)` / `,` / whitespace (from the call's closing syntax) are
/// tolerated so `123)` and `3.14,` are still recognized.
fn is_numeric_literal(arg: &str) -> bool {
    let arg = arg.trim_end_matches([')', ',', ';', ' ', '\t']);
    if arg.is_empty() {
        return false;
    }
    let bytes = arg.as_bytes();
    let start = if bytes[0] == b'-' || bytes[0] == b'+' {
        if bytes.len() == 1 {
            return false;
        }
        1
    } else {
        0
    };
    let mut seen_digit = false;
    let mut seen_dot = false;
    for &b in &bytes[start..] {
        if b.is_ascii_digit() {
            seen_digit = true;
        } else if b == b'.' && !seen_dot {
            seen_dot = true;
        } else {
            return false;
        }
    }
    seen_digit
}

// ---------------------------------------------------------------------------
// VITEST-PREF-001: PreferToBeRule
// ---------------------------------------------------------------------------

pub struct PreferToBeRule;

impl Rule for PreferToBeRule {
    fn id(&self) -> &'static str {
        "VITEST-PREF-001"
    }
    fn name(&self) -> &'static str {
        "PreferToBeRule"
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
        let Ok(source) = std::fs::read_to_string(&module.file_path) else {
            return vec![];
        };

        let mut violations = Vec::new();
        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.contains(".toEqual(") {
                let after = trimmed.split(".toEqual(").nth(1).unwrap_or("");
                let arg = after.split(',').next().unwrap_or(after).trim();
                let is_primitive_like = arg.starts_with('"')
                    || arg.starts_with('\'')
                    || arg.starts_with('`')
                    || is_exact_keyword(arg, "true")
                    || is_exact_keyword(arg, "false")
                    || is_exact_keyword(arg, "null")
                    || is_exact_keyword(arg, "undefined")
                    || is_exact_keyword(arg, "NaN")
                    || is_numeric_literal(arg);
                if is_primitive_like {
                    violations.push(Violation {
                        rule_id: self.id().to_string(),
                        rule_name: self.name().to_string(),
                        severity: self.severity(),
                        category: self.category(),
                        message: "Use toBe() for primitive comparisons instead of toEqual()"
                            .to_string(),
                        file_path: module.file_path.clone(),
                        line: line_idx + 1,
                        col: None,
                        suggestion: Some(
                            "Replace .toEqual() with .toBe() for primitive values".to_string(),
                        ),
                        test_name: None,
                    });
                }
            }
        }

        violations
    }
}

// ---------------------------------------------------------------------------
// VITEST-PREF-002: PreferToContainRule
// ---------------------------------------------------------------------------

pub struct PreferToContainRule;

impl Rule for PreferToContainRule {
    fn id(&self) -> &'static str {
        "VITEST-PREF-002"
    }
    fn name(&self) -> &'static str {
        "PreferToContainRule"
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
        let Ok(source) = std::fs::read_to_string(&module.file_path) else {
            return vec![];
        };

        let mut violations = Vec::new();
        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.contains("expect(") && trimmed.contains(".includes(") {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: "Use toContain() instead of assert on .includes()".to_string(),
                    file_path: module.file_path.clone(),
                    line: line_idx + 1,
                    col: None,
                    suggestion: Some("Replace expect(...includes(val)).toBe(true) with expect(...).toContain(val)".to_string()),
                    test_name: None,
                });
            }
        }

        violations
    }
}

// ---------------------------------------------------------------------------
// VITEST-PREF-003: PreferToHaveLengthRule
// ---------------------------------------------------------------------------

pub struct PreferToHaveLengthRule;

impl Rule for PreferToHaveLengthRule {
    fn id(&self) -> &'static str {
        "VITEST-PREF-003"
    }
    fn name(&self) -> &'static str {
        "PreferToHaveLengthRule"
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
        let Ok(source) = std::fs::read_to_string(&module.file_path) else {
            return vec![];
        };

        let mut violations = Vec::new();
        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            let has_expect = trimmed.contains("expect(");
            let has_length = trimmed.contains(".length");
            let has_size = trimmed.contains(".size")
                && !trimmed.contains("window.size")
                && !trimmed.contains("fontSize")
                && !trimmed.contains("font-size");
            if has_expect && (has_length || has_size) {
                let message = if has_size && !has_length {
                    "Use toHaveLength() instead of asserting on .size (Map/Set)"
                } else {
                    "Use toHaveLength() instead of asserting on .length"
                };
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: message.to_string(),
                    file_path: module.file_path.clone(),
                    line: line_idx + 1,
                    col: None,
                    suggestion: Some(
                        "Replace expect(...length).toBe(N) with expect(...).toHaveLength(N)"
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
// VITEST-PREF-005: PreferSpyOnRule
// ---------------------------------------------------------------------------

pub struct PreferSpyOnRule;

const SPY_ON_ASSIGN_PATTERNS: &[&str] = &["= vi.fn()", "= jest.fn()", "= vi.fn(", "= jest.fn("];

impl Rule for PreferSpyOnRule {
    fn id(&self) -> &'static str {
        "VITEST-PREF-005"
    }
    fn name(&self) -> &'static str {
        "PreferSpyOnRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
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
        let Ok(source) = std::fs::read_to_string(&module.file_path) else {
            return vec![];
        };

        let mut violations = Vec::new();
        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            for pattern in SPY_ON_ASSIGN_PATTERNS {
                if trimmed.contains(pattern) {
                    violations.push(Violation {
                        rule_id: self.id().to_string(),
                        rule_name: self.name().to_string(),
                        severity: self.severity(),
                        category: self.category(),
                        message: "Use vi.spyOn() instead of assigning vi.fn() to an object method"
                            .to_string(),
                        file_path: module.file_path.clone(),
                        line: line_idx + 1,
                        col: None,
                        suggestion: Some(
                            "Replace obj.method = vi.fn() with vi.spyOn(obj, 'method')".to_string(),
                        ),
                        test_name: None,
                    });
                    break;
                }
            }
        }

        violations
    }
}

// ---------------------------------------------------------------------------
// VITEST-PREF-007: PreferCalledOnceRule
// ---------------------------------------------------------------------------

pub struct PreferCalledOnceRule;

impl Rule for PreferCalledOnceRule {
    fn id(&self) -> &'static str {
        "VITEST-PREF-007"
    }
    fn name(&self) -> &'static str {
        "PreferCalledOnceRule"
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
        let Ok(source) = std::fs::read_to_string(&module.file_path) else {
            return vec![];
        };

        let mut violations = Vec::new();
        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.contains("toHaveBeenCalledTimes(1)") {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: "Use toHaveBeenCalledOnce() instead of toHaveBeenCalledTimes(1)"
                        .to_string(),
                    file_path: module.file_path.clone(),
                    line: line_idx + 1,
                    col: None,
                    suggestion: Some(
                        "Replace .toHaveBeenCalledTimes(1) with .toHaveBeenCalledOnce()"
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
// VITEST-PREF-009: PreferHooksOnTopRule
// ---------------------------------------------------------------------------

pub struct PreferHooksOnTopRule;

impl Rule for PreferHooksOnTopRule {
    fn id(&self) -> &'static str {
        "VITEST-PREF-009"
    }
    fn name(&self) -> &'static str {
        "PreferHooksOnTopRule"
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
        let mut violations = Vec::new();

        if module.test_blocks.is_empty() || module.hook_calls.is_empty() {
            return violations;
        }

        let first_test_line = module
            .test_blocks
            .iter()
            .map(|t| t.line)
            .min()
            .unwrap_or(usize::MAX);

        for hook in &module.hook_calls {
            if hook.line > first_test_line {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: format!(
                        "Hook {:?} is defined after test cases — hooks should be placed at the top of the describe block",
                        hook.kind
                    ),
                    file_path: module.file_path.clone(),
                    line: hook.line,
                    col: None,
                    suggestion: Some("Move all hooks above test cases within the describe block".to_string()),
                    test_name: None,
                });
            }
        }

        violations
    }
}

// ---------------------------------------------------------------------------
// VITEST-PREF-010: PreferHooksInOrderRule
// ---------------------------------------------------------------------------

pub struct PreferHooksInOrderRule;

const fn hook_kind_order(kind: HookKind) -> u8 {
    match kind {
        HookKind::BeforeAll => 0,
        HookKind::BeforeEach => 1,
        HookKind::AfterEach => 2,
        HookKind::AfterAll => 3,
    }
}

impl Rule for PreferHooksInOrderRule {
    fn id(&self) -> &'static str {
        "VITEST-PREF-010"
    }
    fn name(&self) -> &'static str {
        "PreferHooksInOrderRule"
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
        let mut violations = Vec::new();

        if module.hook_calls.len() < 2 {
            return violations;
        }

        let mut last_order = hook_kind_order(module.hook_calls[0].kind);

        for hook in &module.hook_calls[1..] {
            let current_order = hook_kind_order(hook.kind);
            if current_order < last_order {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: format!(
                        "Hook {:?} is out of order — hooks should follow: beforeAll → beforeEach → afterEach → afterAll",
                        hook.kind
                    ),
                    file_path: module.file_path.clone(),
                    line: hook.line,
                    col: None,
                    suggestion: Some("Reorder hooks to follow: beforeAll → beforeEach → afterEach → afterAll".to_string()),
                    test_name: None,
                });
            }
            last_order = current_order;
        }

        violations
    }
}

// ---------------------------------------------------------------------------
// VITEST-PREF-012: PreferTodoRule
// ---------------------------------------------------------------------------

pub struct PreferTodoRule;

const EMPTY_TEST_PATTERNS: &[&str] = &[
    "() => {}",
    "() => { }",
    "() => {\n",
    "function() {}",
    "function () {}",
    "() => {  }",
];

impl Rule for PreferTodoRule {
    fn id(&self) -> &'static str {
        "VITEST-PREF-012"
    }
    fn name(&self) -> &'static str {
        "PreferTodoRule"
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
        let Ok(source) = std::fs::read_to_string(&module.file_path) else {
            return vec![];
        };

        let mut violations = Vec::new();

        let test_openers = ["test(", "it(", "test (", "it ("];

        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            let starts_with_test = test_openers.iter().any(|op| trimmed.starts_with(op));
            if !starts_with_test {
                continue;
            }

            for pattern in EMPTY_TEST_PATTERNS {
                if trimmed.contains(pattern) {
                    let name = extract_test_name(trimmed);
                    violations.push(Violation {
                        rule_id: self.id().to_string(),
                        rule_name: self.name().to_string(),
                        severity: self.severity(),
                        category: self.category(),
                        message: "Test has an empty callback — use test.todo() instead".to_string(),
                        file_path: module.file_path.clone(),
                        line: line_idx + 1,
                        col: None,
                        suggestion: Some("Replace empty test with test.todo('...')".to_string()),
                        test_name: name,
                    });
                    break;
                }
            }
        }

        violations
    }
}

fn extract_test_name(line: &str) -> Option<String> {
    let rest = line
        .trim_start_matches("test(")
        .trim_start_matches("it(")
        .trim_start_matches("test (")
        .trim_start_matches("it (")
        .trim();
    if let Some(stripped) = rest.strip_prefix('"') {
        if let Some(end) = stripped.find('"') {
            return Some(stripped[..end].to_string());
        }
    } else if let Some(stripped) = rest.strip_prefix('\'') {
        if let Some(end) = stripped.find('\'') {
            return Some(stripped[..end].to_string());
        }
    } else if let Some(stripped) = rest.strip_prefix('`') {
        if let Some(end) = stripped.find('`') {
            return Some(stripped[..end].to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// VITEST-PREF-013: PreferMockPromiseShorthandRule
// ---------------------------------------------------------------------------

pub struct PreferMockPromiseShorthandRule;

const MOCK_PROMISE_PATTERNS: &[&str] = &[
    "mockReturnValue(Promise.resolve(",
    "mockImplementation(() => Promise.resolve(",
    "mockImplementation(function() { return Promise.resolve(",
    "mockReturnValue(Promise.reject(",
    "mockImplementation(() => Promise.reject(",
    "mockImplementation(function() { return Promise.reject(",
];

impl Rule for PreferMockPromiseShorthandRule {
    fn id(&self) -> &'static str {
        "VITEST-PREF-013"
    }
    fn name(&self) -> &'static str {
        "PreferMockPromiseShorthandRule"
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
        let Ok(source) = std::fs::read_to_string(&module.file_path) else {
            return vec![];
        };

        let mut violations = Vec::new();
        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            for pattern in MOCK_PROMISE_PATTERNS {
                if trimmed.contains(pattern) {
                    let uses_resolve = pattern.contains("resolve");
                    let shorthand = if uses_resolve {
                        "mockResolvedValue"
                    } else {
                        "mockRejectedValue"
                    };
                    violations.push(Violation {
                        rule_id: self.id().to_string(),
                        rule_name: self.name().to_string(),
                        severity: self.severity(),
                        category: self.category(),
                        message: format!(
                            "Use {}() instead of mockReturnValue(Promise.{}(...))",
                            shorthand,
                            if uses_resolve { "resolve" } else { "reject" }
                        ),
                        file_path: module.file_path.clone(),
                        line: line_idx + 1,
                        col: None,
                        suggestion: Some(format!(
                            "Replace mockReturnValue(Promise.{}(val)) with {}(val)",
                            if uses_resolve { "resolve" } else { "reject" },
                            shorthand
                        )),
                        test_name: None,
                    });
                    break;
                }
            }
        }

        violations
    }
}

// ---------------------------------------------------------------------------
// VITEST-PREF-014: PreferExpectResolvesRule
// ---------------------------------------------------------------------------

pub struct PreferExpectResolvesRule;

impl Rule for PreferExpectResolvesRule {
    fn id(&self) -> &'static str {
        "VITEST-PREF-014"
    }
    fn name(&self) -> &'static str {
        "PreferExpectResolvesRule"
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
        let Ok(source) = std::fs::read_to_string(&module.file_path) else {
            return vec![];
        };

        let mut violations = Vec::new();
        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.contains("expect(await ") || trimmed.contains("expect( await ") {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    category: self.category(),
                    message: "Use expect(...).resolves instead of expect(await ...)".to_string(),
                    file_path: module.file_path.clone(),
                    line: line_idx + 1,
                    col: None,
                    suggestion: Some("Replace expect(await promise).toBe(...) with expect(promise).resolves.toBe(...)".to_string()),
                    test_name: None,
                });
            }
        }

        violations
    }
}
