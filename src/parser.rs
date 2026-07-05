use std::path::Path;

use tree_sitter::{Node, Parser};

use crate::models::{
    DescribeBlock, ExpectOutsideTest, ExportEntry, ExportKind, GlobalStub, HookCall, HookKind,
    ImportEntry, LocatorChain, MockScope, ParsedModule, PlaywrightCall, PlaywrightModule,
    SnapshotSize, TestBlock, TestRuntime, ViMockCall,
};

/// Tree-sitter-based TypeScript/TSX parser that extracts test metadata from
/// Vitest test files.
pub struct TsParser;

#[derive(Default)]
struct Context {
    imports: Vec<String>,
    imports_parsed: Vec<ImportEntry>,
    vi_mocks: Vec<ViMockCall>,
    hook_calls: Vec<HookCall>,
    test_blocks: Vec<TestBlock>,
    describe_blocks: Vec<DescribeBlock>,
    expects_outside_tests: Vec<ExpectOutsideTest>,
    imports_node_test: bool,
    snapshot_sizes: Vec<SnapshotSize>,
    runtime: TestRuntime,
    playwright_module: Option<PlaywrightModule>,
    global_stubs: Vec<GlobalStub>,
}

/// Immutable state threaded through the tree-walk that does not vary per call.
struct WalkCtx<'a> {
    source: &'a str,
    path: &'a Path,
    describe_depth: usize,
    scope: MockScope,
}

impl<'a> WalkCtx<'a> {
    fn with_scope(&self, scope: MockScope) -> Self {
        Self {
            source: self.source,
            path: self.path,
            describe_depth: self.describe_depth,
            scope,
        }
    }

    fn with_depth(&self, describe_depth: usize) -> Self {
        Self {
            source: self.source,
            path: self.path,
            describe_depth,
            scope: self.scope,
        }
    }
}

/// Per-call metadata extracted from the callee node.
struct CallInfo<'a> {
    func_name: &'a str,
    full_callee: &'a str,
    is_skip: bool,
    is_only: bool,
    node: Node<'a>,
}

impl TsParser {
    /// Create a new parser instance.
    pub const fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }

    /// Parse a single test file at `path` and return the extracted module
    /// metadata (test blocks, imports, mocks, hooks, etc.).
    pub fn parse_file(&self, path: &Path) -> anyhow::Result<ParsedModule> {
        let mut parser = Parser::new();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let language = if ext == "tsx" || ext == "jsx" {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        } else {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
        };
        parser.set_language(&language)?;

        let source = std::fs::read_to_string(path)?;
        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse file: {}", path.display()))?;

        let root = tree.root_node();
        let mut ctx = Context::default();

        Self::collect(
            root,
            &WalkCtx {
                source: &source,
                path,
                describe_depth: 0,
                scope: MockScope::Module,
            },
            &mut ctx,
        );

        let has_fake_timers = source.contains("useFakeTimers");

        let mut exports = Vec::new();
        Self::collect_exports(root, &source, &mut exports);

        Ok(ParsedModule {
            file_path: path.to_path_buf(),
            imports: ctx.imports,
            imports_parsed: ctx.imports_parsed,
            vi_mocks: ctx.vi_mocks,
            hook_calls: ctx.hook_calls,
            test_blocks: ctx.test_blocks,
            describe_blocks: ctx.describe_blocks,
            has_fake_timers,
            expects_outside_tests: ctx.expects_outside_tests,
            imports_node_test: ctx.imports_node_test,
            snapshot_sizes: ctx.snapshot_sizes,
            exports,
            runtime: ctx.runtime,
            playwright: ctx.playwright_module,
            global_stubs: ctx.global_stubs,
        })
    }

    fn collect(node: Node, walk: &WalkCtx<'_>, ctx: &mut Context) {
        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            match child.kind() {
                "import_statement" => {
                    Self::collect_import_statement(child, walk.source, ctx);
                }
                "call_expression" => {
                    Self::handle_call(child, walk, ctx);
                }
                "expression_statement" => {
                    Self::collect_global_stub_assignment(&child, walk.source, ctx);
                    Self::collect(child, walk, ctx);
                }
                "lexical_declaration" | "variable_declaration" => {
                    Self::collect_global_stub_declaration(&child, walk.source, ctx);
                    Self::collect(child, walk, ctx);
                }
                _ => {
                    Self::collect(child, walk, ctx);
                }
            }
        }
    }

    fn collect_import_statement(child: Node, source: &str, ctx: &mut Context) {
        let text = child.utf8_text(source.as_bytes()).unwrap_or("").to_string();
        ctx.imports.push(text);
        let Some(entry) = Self::parse_import(child, source) else {
            return;
        };
        if entry.source == "node:test" {
            ctx.imports_node_test = true;
        }
        if entry.source == "@playwright/test" {
            ctx.runtime = TestRuntime::Playwright;
            ctx.playwright_module = Some(PlaywrightModule::default());
        } else if entry.source.starts_with("vitest") && ctx.runtime == TestRuntime::Unknown {
            ctx.runtime = TestRuntime::Vitest;
        }
        if entry.source == "axe-playwright" || entry.source == "@axe-core/playwright" {
            if let Some(ref mut pw) = ctx.playwright_module {
                pw.uses_axe = true;
            }
        }
        ctx.imports_parsed.push(entry);
    }

    fn collect_global_stub_assignment(node: &Node, source: &str, ctx: &mut Context) {
        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            if child.kind() == "assignment_expression" {
                let text = child.utf8_text(source.as_bytes()).unwrap_or("");
                let lhs = text.split('=').next().unwrap_or("").trim();
                if lhs.starts_with("global.") || lhs.starts_with("globalThis.") {
                    let target_name = lhs
                        .strip_prefix("global.")
                        .or_else(|| lhs.strip_prefix("globalThis."))
                        .unwrap_or(lhs)
                        .trim()
                        .to_string();
                    ctx.global_stubs.push(GlobalStub {
                        target: target_name,
                        line: node.start_position().row + 1,
                    });
                }
            }
        }
    }

    fn collect_global_stub_declaration(node: &Node, source: &str, ctx: &mut Context) {
        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            if child.kind() != "variable_declarator" {
                continue;
            }
            let text = child.utf8_text(source.as_bytes()).unwrap_or("");
            if !text.contains("vi.fn()") && !text.contains("vi.fn(") {
                continue;
            }
            let Some(name_node) = child.child_by_field_name("name") else {
                continue;
            };
            let name = name_node
                .utf8_text(source.as_bytes())
                .unwrap_or("")
                .to_string();
            if !name.starts_with("global.") && !name.starts_with("globalThis.") {
                continue;
            }
            let target_name = name
                .strip_prefix("global.")
                .or_else(|| name.strip_prefix("globalThis."))
                .unwrap_or(&name)
                .to_string();
            ctx.global_stubs.push(GlobalStub {
                target: target_name,
                line: node.start_position().row + 1,
            });
        }
    }

    fn handle_call(node: Node, walk: &WalkCtx<'_>, ctx: &mut Context) {
        let Some(func_node) = node.child_by_field_name("function") else {
            Self::collect(node, walk, ctx);
            return;
        };

        let (func_name, is_skip, is_only) = Self::parse_callee(func_node, walk.source);
        let full_callee = func_node
            .utf8_text(walk.source.as_bytes())
            .unwrap_or("")
            .to_string();

        let info = CallInfo {
            func_name: &func_name,
            full_callee: &full_callee,
            is_skip,
            is_only,
            node,
        };
        Self::track_expect_outside_test(info.func_name, walk.scope, node, ctx);
        Self::track_snapshot_calls(info.full_callee, node, walk.source, ctx);
        Self::track_vi_mock(info.full_callee, node, walk.source, walk.scope, ctx);
        Self::track_vi_stub_global(info.full_callee, node, walk.source, ctx);
        Self::track_pw_runtime(ctx, info.full_callee, node, walk.source);

        Self::dispatch_call(&info, walk, ctx);
    }

    fn track_expect_outside_test(func_name: &str, scope: MockScope, node: Node, ctx: &mut Context) {
        if scope == MockScope::Module && func_name == "expect" {
            ctx.expects_outside_tests.push(ExpectOutsideTest {
                line: node.start_position().row + 1,
            });
        }
    }

    fn track_snapshot_calls(full_callee: &str, node: Node, source: &str, ctx: &mut Context) {
        if !full_callee.ends_with(".toMatchInlineSnapshot")
            && !full_callee.ends_with(".toMatchSnapshot")
        {
            return;
        }
        let Some(args) = node.child_by_field_name("arguments") else {
            return;
        };
        let Some(first) = args.named_child(0) else {
            return;
        };
        if first.kind() == "string" || first.kind() == "template_string" {
            let content = first.utf8_text(source.as_bytes()).unwrap_or("");
            ctx.snapshot_sizes.push(SnapshotSize {
                line: first.start_position().row + 1,
                size: content.lines().count(),
            });
        }
    }

    fn track_vi_mock(
        full_callee: &str,
        node: Node,
        source: &str,
        scope: MockScope,
        ctx: &mut Context,
    ) {
        if full_callee != "vi.mock" {
            return;
        }
        if let Some(entry) = Self::extract_vi_mock(node, source, scope) {
            ctx.vi_mocks.push(entry);
        }
    }

    fn track_vi_stub_global(full_callee: &str, node: Node, source: &str, ctx: &mut Context) {
        if full_callee != "vi.stubGlobal" {
            return;
        }
        let Some(args) = node.child_by_field_name("arguments") else {
            return;
        };
        let Some(first) = args.named_child(0) else {
            return;
        };
        if let Some(target) = Self::string_value(first, source) {
            ctx.global_stubs.push(GlobalStub {
                target,
                line: node.start_position().row + 1,
            });
        }
    }

    fn track_pw_runtime(ctx: &mut Context, full_callee: &str, node: Node, source: &str) {
        if ctx.runtime == TestRuntime::Playwright {
            Self::track_playwright_call(full_callee, node, source, ctx);
        }
    }

    fn dispatch_call(info: &CallInfo<'_>, walk: &WalkCtx<'_>, ctx: &mut Context) {
        match info.func_name {
            "test" | "it" | "fit" | "xit" => {
                Self::dispatch_test_call(info, walk, ctx);
            }
            "describe" | "fdescribe" | "xdescribe" => {
                Self::add_describe_block(info.node, walk, info.is_only, ctx);
            }
            "beforeEach" | "afterEach" | "beforeAll" | "afterAll" => {
                Self::dispatch_hook_call(info.func_name, info.node, walk, ctx);
            }
            _ => {
                Self::collect(info.node, walk, ctx);
            }
        }
    }

    fn dispatch_test_call(info: &CallInfo<'_>, walk: &WalkCtx<'_>, ctx: &mut Context) {
        if info.full_callee.starts_with("test.describe") {
            Self::add_describe_block(info.node, walk, info.is_only, ctx);
        } else {
            let uses_fit_or_xit =
                info.full_callee.starts_with("fit") || info.full_callee.starts_with("xit");
            if let Some(tb) =
                Self::extract_test(info.node, walk, info.is_skip, info.is_only, uses_fit_or_xit)
            {
                ctx.test_blocks.push(tb);
            }
            if let Some(body) = Self::callback_body(info.node) {
                let inner = walk.with_scope(MockScope::Test);
                Self::collect(body, &inner, ctx);
            }
        }
    }

    fn dispatch_hook_call(func_name: &str, node: Node, walk: &WalkCtx<'_>, ctx: &mut Context) {
        let kind = match func_name {
            "beforeEach" => HookKind::BeforeEach,
            "afterEach" => HookKind::AfterEach,
            "beforeAll" => HookKind::BeforeAll,
            "afterAll" => HookKind::AfterAll,
            _ => return,
        };
        let mut vi_calls = Vec::new();
        if let Some(body) = Self::single_callback_body(node) {
            Self::collect_vi_calls(body, walk.source, &mut vi_calls);
            let inner = walk.with_scope(MockScope::Hook);
            Self::collect(body, &inner, ctx);
        }
        ctx.hook_calls.push(HookCall {
            kind,
            line: node.start_position().row + 1,
            vi_calls,
        });
    }

    fn collect_vi_calls(node: Node, source: &str, out: &mut Vec<String>) {
        if node.kind() == "call_expression" {
            if let Some(func) = node.child_by_field_name("function") {
                let text = func.utf8_text(source.as_bytes()).unwrap_or("");
                if text.starts_with("vi.") {
                    out.push(text.to_string());
                }
            }
        }
        for i in 0..node.named_child_count() {
            if let Some(child) = node.named_child(i) {
                Self::collect_vi_calls(child, source, out);
            }
        }
    }

    fn collect_exports(node: Node, source: &str, exports: &mut Vec<ExportEntry>) {
        if node.kind() != "export_statement" {
            for i in 0..node.named_child_count() {
                if let Some(child) = node.named_child(i) {
                    Self::collect_exports(child, source, exports);
                }
            }
            return;
        }

        let line = node.start_position().row + 1;
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");

        // Check for `export default`
        if text.starts_with("export default") {
            exports.push(ExportEntry {
                name: "default".to_string(),
                kind: ExportKind::Default,
                line,
            });
            return;
        }

        // Check for `export * from`
        if text.starts_with("export *") {
            exports.push(ExportEntry {
                name: "*".to_string(),
                kind: ExportKind::Namespace,
                line,
            });
            return;
        }

        // Check for `export { a, b }` (re-exports or named exports)
        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            match child.kind() {
                "export_clause" => {
                    Self::collect_export_clause(child, source, exports, line);
                }
                "lexical_declaration" | "variable_declaration" => {
                    Self::collect_export_variable(child, source, exports, line);
                }
                "function_declaration" | "class_declaration" | "abstract_class_declaration" => {
                    Self::collect_export_declaration(child, source, exports, line);
                }
                "identifier" => {
                    Self::collect_export_identifier(child, source, exports, line);
                }
                _ => {}
            }
        }
    }

    fn collect_export_clause(
        child: Node,
        source: &str,
        exports: &mut Vec<ExportEntry>,
        line: usize,
    ) {
        for j in 0..child.named_child_count() {
            let Some(spec) = child.named_child(j) else {
                continue;
            };
            if spec.kind() != "export_specifier" {
                continue;
            }
            let name = spec
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or("")
                .to_string();
            if !name.is_empty() {
                exports.push(ExportEntry {
                    name,
                    kind: ExportKind::Named,
                    line,
                });
            }
        }
    }

    fn collect_export_variable(
        child: Node,
        source: &str,
        exports: &mut Vec<ExportEntry>,
        line: usize,
    ) {
        for j in 0..child.named_child_count() {
            let Some(decl) = child.named_child(j) else {
                continue;
            };
            if decl.kind() != "variable_declarator" {
                continue;
            }
            let Some(name_node) = decl.child_by_field_name("name") else {
                continue;
            };
            let name = name_node
                .utf8_text(source.as_bytes())
                .unwrap_or("")
                .to_string();
            if !name.is_empty() {
                exports.push(ExportEntry {
                    name,
                    kind: ExportKind::Named,
                    line,
                });
            }
        }
    }

    fn collect_export_declaration(
        child: Node,
        source: &str,
        exports: &mut Vec<ExportEntry>,
        line: usize,
    ) {
        let name = child
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
            .unwrap_or("")
            .to_string();
        if !name.is_empty() {
            exports.push(ExportEntry {
                name,
                kind: ExportKind::Named,
                line,
            });
        }
    }

    fn collect_export_identifier(
        child: Node,
        source: &str,
        exports: &mut Vec<ExportEntry>,
        line: usize,
    ) {
        let name = child.utf8_text(source.as_bytes()).unwrap_or("").to_string();
        if !name.is_empty() {
            exports.push(ExportEntry {
                name,
                kind: ExportKind::Default,
                line,
            });
        }
    }

    fn track_playwright_call(full_callee: &str, node: Node, source: &str, ctx: &mut Context) {
        let pw_module = match &mut ctx.playwright_module {
            Some(m) => m,
            None => return,
        };

        let line = node.start_position().row + 1;

        Self::track_pw_tracked_calls(full_callee, node, source, line, pw_module);
        Self::track_pw_evaluate(full_callee, node, source, line, pw_module);
        Self::track_pw_axe(full_callee, pw_module);
        Self::track_pw_locator_chains(full_callee, node, source, line, pw_module);
    }

    fn track_pw_tracked_calls(
        full_callee: &str,
        node: Node,
        source: &str,
        line: usize,
        pw_module: &mut PlaywrightModule,
    ) {
        let tracked_calls = [
            "waitForTimeout",
            "page.$",
            "page.$$",
            ".nth",
            ".xpath",
            "setTimeout",
        ];
        for tc in &tracked_calls {
            if full_callee.contains(tc) {
                let raw_arg = if let Some(args) = node.child_by_field_name("arguments") {
                    args.named_child(0)
                        .and_then(|a| Self::string_value(a, source))
                } else {
                    None
                };
                pw_module.calls.push(PlaywrightCall {
                    call_name: full_callee.to_string(),
                    line,
                    raw_arg,
                });
                break;
            }
        }
    }

    fn track_pw_evaluate(
        full_callee: &str,
        node: Node,
        source: &str,
        line: usize,
        pw_module: &mut PlaywrightModule,
    ) {
        if full_callee.contains("evaluate") {
            let text = node.utf8_text(source.as_bytes()).unwrap_or("");
            if text.contains("innerText") {
                pw_module.evaluate_inner_text.push(line);
            }
        }
    }

    fn track_pw_axe(full_callee: &str, pw_module: &mut PlaywrightModule) {
        if full_callee.contains("injectAxe")
            || full_callee.contains("checkA11y")
            || full_callee.contains("AxeBuilder")
        {
            pw_module.uses_axe = true;
        }
    }

    fn track_pw_locator_chains(
        full_callee: &str,
        node: Node,
        source: &str,
        line: usize,
        pw_module: &mut PlaywrightModule,
    ) {
        let locator_roots = [
            "getByRole",
            "getByText",
            "getByTestId",
            "getByPlaceholder",
            "getByLabel",
            "getByAltText",
            "getByTitle",
            "locator",
            "page.locator",
        ];
        for root in &locator_roots {
            if full_callee.contains(root) {
                let raw_arg = if let Some(args) = node.child_by_field_name("arguments") {
                    args.named_child(0)
                        .and_then(|a| Self::string_value(a, source))
                } else {
                    None
                };
                let method = full_callee.split('.').next_back().unwrap_or("").to_string();
                pw_module.locator_chains.push(LocatorChain {
                    root: root.to_string(),
                    raw_arg,
                    method,
                    line,
                });
                break;
            }
        }
    }

    fn extract_vi_mock(node: Node, source: &str, scope: MockScope) -> Option<ViMockCall> {
        let args = node.child_by_field_name("arguments")?;
        if args.named_child_count() == 0 {
            return None;
        }
        let first = args.named_child(0)?;
        // Handle vi.mock("path"), vi.mock(`path`), and vi.mock(import("path"))
        let src = Self::string_value(first, source).or_else(|| {
            // Check for import("path") call expression.
            if first.kind() == "call_expression" {
                if let Some(func) = first.child_by_field_name("function") {
                    if func.kind() == "import" && first.child_by_field_name("arguments").is_some() {
                        let import_args = first.child_by_field_name("arguments")?;
                        let import_first = import_args.named_child(0)?;
                        return Self::string_value(import_first, source);
                    }
                }
            }
            None
        })?;

        // Extract factory keys from second argument if present
        let factory_keys = if args.named_child_count() > 1 {
            let second = args.named_child(1)?;
            Self::extract_factory_keys(second, source)
        } else {
            Vec::new()
        };

        Some(ViMockCall {
            source: src,
            line: node.start_position().row + 1,
            scope,
            factory_keys,
        })
    }

    /// Extract the keys returned by a vi.mock factory function.
    fn extract_factory_keys(node: Node, source: &str) -> Vec<String> {
        let mut keys = Vec::new();

        // The factory is typically an arrow function or function expression
        // vi.mock("path", () => ({ default: "foo", named: "bar" }))
        if node.kind() == "arrow_function" || node.kind() == "function" {
            // Find the return statement
            if let Some(body) = node.child_by_field_name("body") {
                Self::collect_returned_keys(body, source, &mut keys);
            }
        }

        keys
    }

    /// Recursively collect keys from object literals in return statements.
    fn collect_returned_keys(node: Node, source: &str, keys: &mut Vec<String>) {
        match node.kind() {
            "object" | "object_pattern" => {
                Self::collect_object_keys(node, source, keys);
            }
            "statement_block" | "return_statement" | "parenthesized_expression" => {
                for i in 0..node.named_child_count() {
                    if let Some(child) = node.named_child(i) {
                        Self::collect_returned_keys(child, source, keys);
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_object_keys(node: Node, source: &str, keys: &mut Vec<String>) {
        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            if child.kind() == "pair" || child.kind() == "property" {
                if let Some(key) = child.child_by_field_name("key") {
                    if let Ok(text) = key.utf8_text(source.as_bytes()) {
                        keys.push(text.to_string());
                    }
                }
            } else if child.kind() == "shorthand_property_identifier" {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    keys.push(text.to_string());
                }
            }
        }
    }

    fn parse_import(node: Node, source: &str) -> Option<ImportEntry> {
        // tree-sitter-typescript: import_statement has a `source` field
        // (string literal). The clause is one of: identifier, namespace_import,
        // named_imports — we walk the named children to find them.
        let mut entry = ImportEntry {
            source: String::new(),
            named: Vec::new(),
            default: None,
            namespace: None,
            line: node.start_position().row + 1,
        };

        for i in 0..node.named_child_count() {
            let child = node.named_child(i)?;
            match child.kind() {
                "string" => {
                    let raw = child.utf8_text(source.as_bytes()).unwrap_or("");
                    entry.source = raw
                        .trim_matches(|c: char| c == '"' || c == '\'' || c == '`')
                        .to_string();
                }
                "import_clause" => {
                    Self::walk_import_clause(child, source, &mut entry);
                }
                _ => {}
            }
        }

        if entry.source.is_empty() {
            return None;
        }
        Some(entry)
    }

    fn walk_import_clause(node: Node, source: &str, entry: &mut ImportEntry) {
        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            match child.kind() {
                "identifier" => {
                    Self::walk_import_identifier(child, source, entry);
                }
                "namespace_import" => {
                    Self::walk_import_namespace(child, source, entry);
                }
                "named_imports" => {
                    Self::walk_import_named(child, source, entry);
                }
                _ => {}
            }
        }
    }

    fn walk_import_identifier(child: Node, source: &str, entry: &mut ImportEntry) {
        let name = child.utf8_text(source.as_bytes()).unwrap_or("").to_string();
        if !name.is_empty() {
            entry.default = Some(name);
        }
    }

    fn walk_import_namespace(child: Node, source: &str, entry: &mut ImportEntry) {
        for j in 0..child.named_child_count() {
            if let Some(inner) = child.named_child(j) {
                if inner.kind() == "identifier" {
                    entry.namespace =
                        Some(inner.utf8_text(source.as_bytes()).unwrap_or("").to_string());
                }
            }
        }
    }

    fn walk_import_named(child: Node, source: &str, entry: &mut ImportEntry) {
        for j in 0..child.named_child_count() {
            let Some(spec) = child.named_child(j) else {
                continue;
            };
            if spec.kind() != "import_specifier" {
                continue;
            }
            let name = spec
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or("");
            if name.is_empty() {
                Self::walk_import_named_fallback(spec, source, entry);
            } else {
                entry.named.push(name.to_string());
            }
        }
    }

    fn walk_import_named_fallback(spec: Node, source: &str, entry: &mut ImportEntry) {
        for k in 0..spec.named_child_count() {
            if let Some(c) = spec.named_child(k) {
                if c.kind() == "identifier" {
                    let n = c.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                    if !n.is_empty() {
                        entry.named.push(n);
                        break;
                    }
                }
            }
        }
    }

    fn parse_callee(node: Node, source: &str) -> (String, bool, bool) {
        match node.kind() {
            "identifier" => {
                let name = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                (name, false, false)
            }
            "member_expression" => {
                let full = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                let segments = full.split('.').collect::<Vec<_>>();
                let base = segments.first().copied().unwrap_or("").to_string();
                let is_skip = segments.iter().any(|s| matches!(*s, "skip" | "todo"));
                let is_only = segments.contains(&"only");
                (base, is_skip, is_only)
            }
            _ => (String::new(), false, false),
        }
    }

    fn find_callback_in_args(args: Node) -> Option<Node> {
        for i in 0..args.named_child_count() {
            let child = args.named_child(i)?;
            if matches!(child.kind(), "arrow_function" | "function_expression") {
                return Some(child);
            }
        }
        None
    }

    fn callback_body(call_node: Node) -> Option<Node> {
        let args = call_node.child_by_field_name("arguments")?;
        let callback = Self::find_callback_in_args(args)?;
        Self::func_body(callback)
    }

    fn single_callback_body(call_node: Node) -> Option<Node> {
        let args = call_node.child_by_field_name("arguments")?;
        if args.named_child_count() == 0 {
            return None;
        }
        let callback = args.named_child(0)?;
        Self::func_body(callback)
    }

    fn add_describe_block(node: Node, walk: &WalkCtx<'_>, is_only: bool, ctx: &mut Context) {
        let name_node = node
            .child_by_field_name("arguments")
            .and_then(|args| args.named_child(0));
        let name = name_node
            .and_then(|n| Self::string_value(n, walk.source))
            .unwrap_or_default();
        let title_is_template_literal = name_node.is_some_and(|n| n.kind() == "template_string");
        let title_is_empty = name.is_empty();
        let is_async = node
            .child_by_field_name("arguments")
            .and_then(|args| Self::find_callback_in_args(args))
            .is_some_and(|cb| {
                let text = cb.utf8_text(walk.source.as_bytes()).unwrap_or("");
                text.trim_start().starts_with("async")
            });
        ctx.describe_blocks.push(DescribeBlock {
            name,
            file_path: walk.path.to_path_buf(),
            line: node.start_position().row + 1,
            is_only,
            depth: walk.describe_depth,
            title_is_template_literal,
            title_is_empty,
            is_async,
        });
        if let Some(body) = Self::callback_body(node) {
            let inner = walk.with_depth(walk.describe_depth + 1);
            Self::collect(body, &inner, ctx);
        } else {
            Self::collect(node, walk, ctx);
        }
    }

    fn func_body(func_node: Node) -> Option<Node> {
        if func_node.kind() != "arrow_function" && func_node.kind() != "function_expression" {
            return None;
        }
        for i in 0..func_node.named_child_count() {
            let child = func_node.named_child(i).unwrap();
            if child.kind() == "statement_block" {
                return Some(child);
            }
        }
        func_node.child_by_field_name("body")
    }

    fn extract_test(
        node: Node,
        walk: &WalkCtx<'_>,
        is_skip: bool,
        is_only: bool,
        uses_fit_or_xit: bool,
    ) -> Option<TestBlock> {
        let args = node.child_by_field_name("arguments")?;
        if args.named_child_count() < 1 {
            return None;
        }

        let name_node = args.named_child(0)?;
        // Non-string test names (e.g. `test(123)`, `test(someVar)`) cannot be
        // resolved to a literal via `string_value`. For dynamic identifier or
        // numeric names, fall back to the raw node text instead of dropping
        // the test from analysis entirely. Other kinds (arrays, ERROR nodes
        // from malformed parses, etc.) still bail to preserve prior behavior.
        let name = match Self::string_value(name_node, walk.source) {
            Some(s) => s,
            None => match name_node.kind() {
                "identifier" | "number" => name_node
                    .utf8_text(walk.source.as_bytes())
                    .ok()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "<non-string>".to_string()),
                _ => return None,
            },
        };

        let body = Self::find_callback_in_args(args).and_then(Self::func_body);

        let st = body.map_or_else(Analysis::default, |b| Self::analyze(b, walk.source));

        let title_is_template_literal = args
            .named_child(0)
            .is_some_and(|n| n.kind() == "template_string");

        // Detect done callback pattern (parameter named "done" in test callback)
        let has_done_callback = node
            .child_by_field_name("arguments")
            .and_then(|args| Self::find_callback_in_args(args))
            .is_some_and(|cb| Self::has_done_param(cb, walk.source));

        Some(TestBlock {
            name,
            file_path: walk.path.to_path_buf(),
            line: node.start_position().row + 1,
            has_assertions: st.assertion_count > 0,
            assertion_count: st.assertion_count,
            has_conditional_logic: st.has_conditional,
            has_try_catch: st.has_try_catch,
            uses_settimeout: st.uses_settimeout,
            uses_promise_settimeout: st.uses_promise_settimeout,
            uses_datemock: st.uses_datemock,
            has_multiple_expects: st.assertion_count > 1,
            is_skipped: is_skip,
            is_only,
            is_nested: walk.describe_depth > 3,
            has_return_statement: st.has_return,
            unawaited_async_assertions: st.unawaited_async_assertions,
            uses_fake_timers: st.uses_fake_timers,
            uses_random: st.uses_random,
            has_expect_call_without_assertion: st.has_expect_call_without_assertion,
            has_return_of_expect: st.has_return_of_expect,
            title_is_template_literal,
            has_async_expect_wrapper: st.has_async_expect_wrapper,
            uses_fit_or_xit,
            has_done_callback,
            has_conditional_expect: st.has_conditional_expect,
            weak_assertion_count: st.weak_assertion_count,
            has_real_timers_call: st.has_real_timers_call,
        })
    }

    fn has_done_param(cb: Node, source: &str) -> bool {
        if cb.kind() != "arrow_function" && cb.kind() != "function_expression" {
            return false;
        }
        let Some(params) = Self::find_params_node(cb) else {
            return false;
        };
        Self::check_done_in_params(params, source)
    }

    fn find_params_node(cb: Node) -> Option<Node> {
        cb.child_by_field_name("parameters").or_else(|| {
            for i in 0..cb.named_child_count() {
                let child = cb.named_child(i)?;
                if child.kind() == "formal_parameters" {
                    return Some(child);
                }
            }
            None
        })
    }

    fn check_done_in_params(params: Node, source: &str) -> bool {
        for i in 0..params.named_child_count() {
            let Some(param) = params.named_child(i) else {
                continue;
            };
            if Self::is_done_identifier(param, source) {
                return true;
            }
        }
        false
    }

    fn is_done_identifier(param: Node, source: &str) -> bool {
        match param.kind() {
            "identifier" => param.utf8_text(source.as_bytes()).unwrap_or("") == "done",
            "required_parameter" => {
                for j in 0..param.named_child_count() {
                    if let Some(inner) = param.named_child(j) {
                        if inner.kind() == "identifier"
                            && inner.utf8_text(source.as_bytes()).unwrap_or("") == "done"
                        {
                            return true;
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn string_value(node: Node, source: &str) -> Option<String> {
        match node.kind() {
            "string" => {
                let text = node.utf8_text(source.as_bytes()).unwrap_or("");
                Some(
                    text.trim_matches(|c| c == '"' || c == '\'' || c == '`')
                        .to_string(),
                )
            }
            "template_string" => {
                // Reject templates with interpolations (${...}).
                for i in 0..node.named_child_count() {
                    if let Some(child) = node.named_child(i) {
                        if child.kind() == "template_substitution" {
                            return None;
                        }
                    }
                }
                let text = node.utf8_text(source.as_bytes()).unwrap_or("");
                Some(text.trim_matches('`').to_string())
            }
            _ => None,
        }
    }

    fn is_awaited(node: Node) -> bool {
        let mut curr = node;
        while let Some(parent) = curr.parent() {
            if parent.kind() == "await_expression" {
                return true;
            }
            if parent.kind() == "expression_statement"
                || parent.kind() == "lexical_declaration"
                || parent.kind() == "variable_declaration"
                || parent.kind() == "statement_block"
            {
                break;
            }
            curr = parent;
        }
        false
    }

    fn analyze(node: Node, source: &str) -> Analysis {
        let mut st = Analysis::default();
        Self::walk_body(node, source, &mut st);
        st
    }

    fn walk_body(node: Node, source: &str, st: &mut Analysis) {
        match node.kind() {
            "call_expression" => {
                Self::walk_call_expression(node, source, st);
                return;
            }
            "new_expression" => {
                Self::walk_new_expression(node, source, st);
            }
            "if_statement" | "switch_statement" => {
                Self::walk_conditional(node, source, st);
                return;
            }
            "try_statement" => {
                st.has_try_catch = true;
            }
            "return_statement" => {
                Self::walk_return_statement(node, source, st);
            }
            _ => {}
        }

        Self::walk_body_children(node, source, st);
    }

    fn walk_call_expression(node: Node, source: &str, st: &mut Analysis) {
        let Some(func) = node.child_by_field_name("function") else {
            return;
        };
        let text = func.utf8_text(source.as_bytes()).unwrap_or("");
        let is_expect_call = func.kind() == "identifier" && text == "expect";

        if is_expect_call {
            Self::handle_expect_call(node, source, st);
        } else {
            Self::walk_body(func, source, st);
        }

        Self::walk_call_flags(node, text, st);

        let Some(args) = node.child_by_field_name("arguments") else {
            return;
        };
        for i in 0..args.named_child_count() {
            let Some(child) = args.named_child(i) else {
                continue;
            };
            Self::walk_body(child, source, st);
        }
    }

    fn handle_expect_call(node: Node, source: &str, st: &mut Analysis) {
        st.assertion_count += 1;
        if st.in_conditional {
            st.has_conditional_expect = true;
        }
        let has_chained_assertion = Self::has_parent_member_call(node);
        if !has_chained_assertion {
            st.has_expect_call_without_assertion = true;
        }
        if let Some((matcher, negated)) = Self::expect_matcher_info(node, source) {
            let is_weak_matcher = Self::WEAK_MATCHERS.contains(&matcher);
            let is_negated_throw = negated && matcher == "toThrow";
            if is_weak_matcher || is_negated_throw {
                st.weak_assertion_count += 1;
            }
        }
        Self::check_async_expect_wrapper(node, source, st);
    }

    fn check_async_expect_wrapper(node: Node, source: &str, st: &mut Analysis) {
        let Some(args) = node.child_by_field_name("arguments") else {
            return;
        };
        let Some(first_arg) = args.named_child(0) else {
            return;
        };
        if first_arg.kind() == "arrow_function" || first_arg.kind() == "function_expression" {
            let func_text = first_arg.utf8_text(source.as_bytes()).unwrap_or("");
            if func_text.trim_start().starts_with("async") {
                st.has_async_expect_wrapper = true;
            }
        }
    }

    fn walk_call_flags(node: Node, text: &str, st: &mut Analysis) {
        if (text.contains(".resolves") || text.contains(".rejects")) && !Self::is_awaited(node) {
            st.unawaited_async_assertions += 1;
        }
        if text == "setTimeout" {
            st.uses_settimeout = true;
            if st.in_promise_constructor {
                st.uses_promise_settimeout = true;
            }
        }
        if text.starts_with("Date.") {
            st.uses_datemock = true;
        }
        if text == "vi.useFakeTimers" {
            st.uses_fake_timers = true;
        }
        if text == "vi.useRealTimers" {
            st.has_real_timers_call = true;
        }
        if text == "Math.random" || text == "crypto.randomUUID" {
            st.uses_random = true;
        }
    }

    fn walk_new_expression(node: Node, source: &str, st: &mut Analysis) {
        let Some(ctor) = node.child_by_field_name("constructor") else {
            return;
        };
        let ctor_text = ctor.utf8_text(source.as_bytes()).unwrap_or("");
        if ctor_text == "Date" {
            st.uses_datemock = true;
        }
        let prev_in_promise_constructor = st.in_promise_constructor;
        if ctor_text == "Promise" {
            st.in_promise_constructor = true;
        }
        if let Some(args) = node.child_by_field_name("arguments") {
            for i in 0..args.named_child_count() {
                let Some(child) = args.named_child(i) else {
                    continue;
                };
                Self::walk_body(child, source, st);
            }
        }
        st.in_promise_constructor = prev_in_promise_constructor;
    }

    fn walk_conditional(node: Node, source: &str, st: &mut Analysis) {
        st.has_conditional = true;
        let prev = st.in_conditional;
        st.in_conditional = true;
        for i in 0..node.named_child_count() {
            let child = node.named_child(i).unwrap();
            Self::walk_body(child, source, st);
        }
        st.in_conditional = prev;
    }

    fn walk_return_statement(node: Node, source: &str, st: &mut Analysis) {
        st.has_return = true;
        for i in 0..node.named_child_count() {
            let child = node.named_child(i).unwrap();
            if Self::contains_expect_call(child, source) {
                st.has_return_of_expect = true;
                break;
            }
        }
    }

    fn walk_body_children(node: Node, source: &str, st: &mut Analysis) {
        for i in 0..node.named_child_count() {
            let child = node.named_child(i).unwrap();
            Self::walk_body(child, source, st);
        }
    }

    /// Check if a node is an `expect()` call inside a member expression chain
    /// (e.g., expect(x).toBe(y) — the expect call has a parent `member_expression`
    /// which is inside another `call_expression`).
    fn has_parent_member_call(node: Node) -> bool {
        let mut curr = node;
        while let Some(parent) = curr.parent() {
            if parent.kind() == "member_expression" {
                // Check if this member_expression is the function of a call_expression.
                if let Some(grandparent) = parent.parent() {
                    if grandparent.kind() == "call_expression" {
                        return true;
                    }
                }
            }
            curr = parent;
        }
        false
    }

    /// Extract the final chained matcher name from an `expect()` call node,
    /// along with whether it's negated (e.g. `.not.toThrow()`).
    /// For `expect(x).toBeDefined()`, returns `("toBeDefined", false)`.
    /// For `expect(x).not.toBe(2)`, returns `("toBe", true)`.
    /// For `expect(() => fn()).not.toThrow()`, returns `("toThrow", true)`.
    fn expect_matcher_info<'a>(expect_node: Node, source: &'a str) -> Option<(&'a str, bool)> {
        let (curr, has_not) = Self::walk_up_chain(expect_node, source)?;
        Self::extract_matcher_from_call(curr, source, has_not)
    }

    /// Walk up from `expect_node` to find the outermost call_expression in the chain.
    /// Handles patterns like: expect(x).toBe(y), expect(x).not.toThrow(), etc.
    fn walk_up_chain<'a>(node: Node<'a>, source: &'a str) -> Option<(Node<'a>, bool)> {
        let mut curr = node;
        let mut has_not = false;
        loop {
            let parent = curr.parent()?;
            if parent.kind() == "member_expression" {
                if Self::is_not_property(parent, source) {
                    has_not = true;
                }
                let grandparent = parent.parent()?;
                if grandparent.kind() == "call_expression"
                    || grandparent.kind() == "member_expression"
                {
                    curr = grandparent;
                    continue;
                }
            } else if parent.kind() == "call_expression" {
                let grandparent = parent.parent()?;
                if grandparent.kind() == "member_expression" {
                    curr = grandparent;
                    continue;
                }
                curr = parent;
            }
            break;
        }
        Some((curr, has_not))
    }

    fn is_not_property(node: Node, source: &str) -> bool {
        node.child_by_field_name("property")
            .is_some_and(|prop| prop.utf8_text(source.as_bytes()).unwrap_or("") == "not")
    }

    fn extract_matcher_from_call<'a>(
        curr: Node,
        source: &'a str,
        has_not: bool,
    ) -> Option<(&'a str, bool)> {
        if curr.kind() == "call_expression" {
            if let Some(func) = curr.child_by_field_name("function") {
                if func.kind() == "member_expression" {
                    if let Some(prop) = func.child_by_field_name("property") {
                        let matcher = prop.utf8_text(source.as_bytes()).unwrap_or("");
                        if matcher == "not" {
                            return None;
                        }
                        return Some((matcher, has_not));
                    }
                }
            }
        }
        None
    }

    const WEAK_MATCHERS: &[&str] = &[
        "toBeDefined",
        "toBeUndefined",
        "toBeTruthy",
        "toBeFalsy",
        "toBeNull",
        "toMatchObject",
        "toHaveProperty",
        "toHaveBeenCalled",
        "toBeCalled",
        "toHaveReturned",
        "toHaveReturnedTimes",
    ];

    /// Check if a subtree contains an `expect()` call.
    fn contains_expect_call(node: Node, source: &str) -> bool {
        if node.kind() == "call_expression" {
            if let Some(func) = node.child_by_field_name("function") {
                if func.kind() == "identifier" {
                    let text = func.utf8_text(source.as_bytes()).unwrap_or("");
                    if text == "expect" {
                        return true;
                    }
                }
            }
        }
        for i in 0..node.named_child_count() {
            let child = node.named_child(i).unwrap();
            if Self::contains_expect_call(child, source) {
                return true;
            }
        }
        false
    }
}

#[derive(Default)]
struct Analysis {
    assertion_count: usize,
    has_conditional: bool,
    has_try_catch: bool,
    uses_settimeout: bool,
    uses_promise_settimeout: bool,
    in_promise_constructor: bool,
    uses_datemock: bool,
    has_return: bool,
    unawaited_async_assertions: usize,
    uses_fake_timers: bool,
    uses_random: bool,
    has_expect_call_without_assertion: bool,
    has_return_of_expect: bool,
    has_async_expect_wrapper: bool,
    has_conditional_expect: bool,
    in_conditional: bool,
    weak_assertion_count: usize,
    has_real_timers_call: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp(content: &str, name: &str) -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        dir
    }

    #[test]
    fn parse_simple_test_file() {
        let dir = write_temp(
            r#"
import { test, expect } from 'vitest';

test('adds numbers', () => {
    expect(1 + 1).toBe(2);
});
"#,
            "simple.test.ts",
        );
        let path = dir.path().join("simple.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_blocks.len(), 1);
        assert_eq!(module.test_blocks[0].name, "adds numbers");
        assert!(module.test_blocks[0].has_assertions);
        assert_eq!(module.test_blocks[0].assertion_count, 1);
        assert!(!module.test_blocks[0].is_skipped);
        assert!(!module.test_blocks[0].is_nested);
    }

    #[test]
    fn parse_detects_fake_timers() {
        let dir = write_temp(
            r#"
import { test, expect, vi } from 'vitest';

test('with fake timers', () => {
    vi.useFakeTimers();
    expect(true).toBe(true);
});
"#,
            "fake.test.ts",
        );
        let path = dir.path().join("fake.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert!(module.has_fake_timers);
    }

    #[test]
    fn parse_skipped_test() {
        let dir = write_temp(
            r#"
import { test, expect } from 'vitest';

test.skip('skipped', () => {
    expect(1).toBe(1);
});
"#,
            "skip.test.ts",
        );
        let path = dir.path().join("skip.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_blocks.len(), 1);
        assert!(module.test_blocks[0].is_skipped);
    }

    #[test]
    fn parse_callee_skip_is_segment_exact_not_substring() {
        // Guards against a substring regression: `.contains(".skip")` would
        // wrongly mark `test.skipReason(...)` as a skipped test. The skip/todo
        // flags must match a full segment exactly.
        let dir = write_temp(
            r#"
import { test, expect } from 'vitest';

test.skipReason('not a real skip modifier', () => {
    expect(1).toBe(1);
});
"#,
            "skip-substring.test.ts",
        );
        let path = dir.path().join("skip-substring.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(
            module.test_blocks.len(),
            1,
            "test.skipReason(...) should still be parsed as a test block"
        );
        assert!(
            !module.test_blocks[0].is_skipped,
            "test.skipReason must NOT set is_skipped — segment match is exact, not substring"
        );
    }

    #[test]
    fn parse_nested_describe() {
        let dir = write_temp(
            r#"
import { describe, test, expect } from 'vitest';

describe('level1', () => {
    describe('level2', () => {
        describe('level3', () => {
            describe('level4', () => {
                test('deeply nested', () => {
                    expect(1).toBe(1);
                });
            });
        });
    });
});
"#,
            "nested.test.ts",
        );
        let path = dir.path().join("nested.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_blocks.len(), 1);
        assert!(module.test_blocks[0].is_nested);
    }

    #[test]
    fn parse_imports() {
        let dir = write_temp(
            r#"
import { test, expect } from 'vitest';
import axios from 'axios';
"#,
            "imports.test.ts",
        );
        let path = dir.path().join("imports.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert!(module.imports.iter().any(|i| i.contains("axios")));
        assert!(module.imports.iter().any(|i| i.contains("vitest")));
    }

    #[test]
    fn parse_describe_with_extra_args() {
        let dir = write_temp(
            r#"
import { describe, test, expect } from 'vitest';

describe('with extra', () => {
    test('inside', () => {
        expect(1).toBe(1);
    });
}, extraConfig);
"#,
            "extra.test.ts",
        );
        let path = dir.path().join("extra.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_blocks.len(), 1);
        assert_eq!(module.test_blocks[0].name, "inside");
        assert!(module.test_blocks[0].has_assertions);
    }

    #[test]
    fn parse_test_name_only_no_callback() {
        let dir = write_temp(
            r#"
import { test } from 'vitest';

test('name only');
"#,
            "nameonly.test.ts",
        );
        let path = dir.path().join("nameonly.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_blocks.len(), 1);
        assert_eq!(module.test_blocks[0].name, "name only");
        assert!(!module.test_blocks[0].has_assertions);
    }

    #[test]
    fn parse_test_with_function_expression() {
        let dir = write_temp(
            r#"
import { test, expect } from 'vitest';

test('function expr', function() {
    expect(1).toBe(1);
});
"#,
            "funcexpr.test.ts",
        );
        let path = dir.path().join("funcexpr.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_blocks.len(), 1);
        assert!(module.test_blocks[0].has_assertions);
        assert_eq!(module.test_blocks[0].assertion_count, 1);
    }

    #[test]
    fn parse_single_describe_not_nested() {
        let dir = write_temp(
            r#"
import { describe, test, expect } from 'vitest';

describe('only one level', () => {
    test('not nested', () => {
        expect(1).toBe(1);
    });
});
"#,
            "single.test.ts",
        );
        let path = dir.path().join("single.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_blocks.len(), 1);
        assert!(!module.test_blocks[0].is_nested);
    }

    #[test]
    fn parse_line_number_correct() {
        let dir = write_temp(
            r#"import { test, expect } from 'vitest';

test('line check', () => {
    expect(1).toBe(1);
});"#,
            "line.test.ts",
        );
        let path = dir.path().join("line.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_blocks.len(), 1);
        assert_eq!(module.test_blocks[0].line, 3);
    }

    #[test]
    fn parse_single_assertion_not_multiple() {
        let dir = write_temp(
            r#"
import { test, expect } from 'vitest';

test('one assert', () => {
    expect(1).toBe(1);
});
"#,
            "one.test.ts",
        );
        let path = dir.path().join("one.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_blocks.len(), 1);
        assert_eq!(module.test_blocks[0].assertion_count, 1);
        assert!(!module.test_blocks[0].has_multiple_expects);
    }

    #[test]
    fn parse_two_assertions_is_multiple() {
        let dir = write_temp(
            r#"
import { test, expect } from 'vitest';

test('two asserts', () => {
    expect(1).toBe(1);
    expect(2).toBe(2);
});
"#,
            "two.test.ts",
        );
        let path = dir.path().join("two.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_blocks.len(), 1);
        assert_eq!(module.test_blocks[0].assertion_count, 2);
        assert!(module.test_blocks[0].has_multiple_expects);
    }

    #[test]
    fn parse_tsx_file_with_jsx() {
        let dir = write_temp(
            r#"
import { render, screen } from '@testing-library/react';
import { test, expect } from 'vitest';
import MyComponent from './MyComponent';

test('renders label', () => {
    render(<MyComponent />);
    expect(screen.getByText('hello')).toBeTruthy();
});
"#,
            "component.test.tsx",
        );
        let path = dir.path().join("component.test.tsx");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_blocks.len(), 1);
        assert_eq!(module.test_blocks[0].name, "renders label");
        assert!(module.test_blocks[0].has_assertions);
    }

    #[test]
    fn parse_deeply_nested_describe_with_extra_args() {
        let dir = write_temp(
            r#"
import { describe, test, expect } from 'vitest';

describe('level1', () => {
    describe('level2', () => {
        describe('level3', () => {
            describe('level4', () => {
                test('deep', () => {
                    expect(1).toBe(1);
                });
            });
        });
    });
}, config);
"#,
            "deep.test.ts",
        );
        let path = dir.path().join("deep.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_blocks.len(), 1);
        assert!(
            module.test_blocks[0].is_nested,
            "test inside 4-level nested describe should be is_nested"
        );
        assert!(module.test_blocks[0].has_assertions);
    }

    #[test]
    fn parse_vi_mock_module_scope() {
        let dir = write_temp(
            r#"
import { vi } from 'vitest';

vi.mock('../infrastructure/database', () => ({ db: {} }));
"#,
            "mock.test.ts",
        );
        let path = dir.path().join("mock.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.vi_mocks.len(), 1);
        assert_eq!(module.vi_mocks[0].source, "../infrastructure/database");
        assert_eq!(module.vi_mocks[0].scope, MockScope::Module);
    }

    #[test]
    fn parse_imports_structured_named_default_namespace() {
        let dir = write_temp(
            r#"
import { test, expect } from 'vitest';
import axios from 'axios';
import * as fs from 'fs';
import { progressPersistence } from './progress-persistence';
"#,
            "structured.test.ts",
        );
        let path = dir.path().join("structured.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        let vitest = module
            .imports_parsed
            .iter()
            .find(|e| e.source == "vitest")
            .unwrap();
        assert!(vitest.named.contains(&"test".to_string()));
        assert!(vitest.named.contains(&"expect".to_string()));

        let axios = module
            .imports_parsed
            .iter()
            .find(|e| e.source == "axios")
            .unwrap();
        assert_eq!(axios.default.as_deref(), Some("axios"));

        let fs_imp = module
            .imports_parsed
            .iter()
            .find(|e| e.source == "fs")
            .unwrap();
        assert_eq!(fs_imp.namespace.as_deref(), Some("fs"));

        let pp = module
            .imports_parsed
            .iter()
            .find(|e| e.source == "./progress-persistence")
            .unwrap();
        assert!(pp.named.contains(&"progressPersistence".to_string()));
    }

    #[test]
    fn parse_hook_calls_capture_vi_methods() {
        let dir = write_temp(
            r#"
import { beforeEach, afterEach, vi } from 'vitest';

beforeEach(() => {
    vi.resetModules();
    vi.restoreAllMocks();
});

afterEach(() => {
    vi.clearAllMocks();
});
"#,
            "hooks.test.ts",
        );
        let path = dir.path().join("hooks.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.hook_calls.len(), 2);
        let before = module
            .hook_calls
            .iter()
            .find(|h| h.kind == HookKind::BeforeEach)
            .unwrap();
        assert!(before.vi_calls.iter().any(|c| c == "vi.resetModules"));
        assert!(before.vi_calls.iter().any(|c| c == "vi.restoreAllMocks"));
        let after = module
            .hook_calls
            .iter()
            .find(|h| h.kind == HookKind::AfterEach)
            .unwrap();
        assert!(after.vi_calls.iter().any(|c| c == "vi.clearAllMocks"));
    }

    #[test]
    fn parse_vi_mock_dynamic_import() {
        let dir = write_temp(
            r#"
import { vi } from 'vitest';

vi.mock(import('../infrastructure/database'));
"#,
            "dynmock.test.ts",
        );
        let path = dir.path().join("dynmock.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.vi_mocks.len(), 1);
        assert_eq!(module.vi_mocks[0].source, "../infrastructure/database");
        assert_eq!(module.vi_mocks[0].scope, MockScope::Module);
    }

    #[test]
    fn parse_vi_mock_exact_integration_fixture() {
        let dir = write_temp(
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
            "my-module2.test.ts",
        );
        let path = dir.path().join("my-module2.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(
            module.vi_mocks.len(),
            1,
            "Expected 1 vi.mock(), got {}",
            module.vi_mocks.len()
        );
        assert_eq!(module.vi_mocks[0].source, "./my-module2");
        assert_eq!(module.vi_mocks[0].factory_keys, vec!["foo", "nonexistent"]);
    }

    #[test]
    fn parse_vi_mock_template_interpolation_ignored() {
        let dir = write_temp(
            r#"
import { vi } from 'vitest';

vi.mock(`../${name}`);
"#,
            "interp.test.ts",
        );
        let path = dir.path().join("interp.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.vi_mocks.len(), 0);
    }

    #[test]
    fn parse_source_module_named_exports() {
        let dir = write_temp(
            r#"
export const calculateTotal = (items: number[]) => items.reduce((a, b) => a + b, 0);
export function formatCurrency(amount: number): string {
    return `$${amount.toFixed(2)}`;
}
export class UserService {}
"#,
            "utils.ts",
        );
        let path = dir.path().join("utils.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.exports.len(), 3);
        assert!(module
            .exports
            .iter()
            .any(|e| e.name == "calculateTotal" && e.kind == ExportKind::Named));
        assert!(module
            .exports
            .iter()
            .any(|e| e.name == "formatCurrency" && e.kind == ExportKind::Named));
        assert!(module
            .exports
            .iter()
            .any(|e| e.name == "UserService" && e.kind == ExportKind::Named));
    }

    #[test]
    fn parse_source_module_default_export() {
        let dir = write_temp(
            r#"
export default function app() {}
"#,
            "app.ts",
        );
        let path = dir.path().join("app.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.exports.len(), 1);
        assert_eq!(module.exports[0].kind, ExportKind::Default);
    }

    #[test]
    fn parse_source_module_re_exports() {
        let dir = write_temp(
            r#"
export { foo, bar } from './other';
"#,
            "reexport.ts",
        );
        let path = dir.path().join("reexport.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.exports.len(), 2);
        assert!(module.exports.iter().any(|e| e.name == "foo"));
        assert!(module.exports.iter().any(|e| e.name == "bar"));
    }

    #[test]
    fn parse_source_module_namespace_export() {
        let dir = write_temp(
            r#"
export * from './utils';
"#,
            "barrel.ts",
        );
        let path = dir.path().join("barrel.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.exports.len(), 1);
        assert_eq!(module.exports[0].kind, ExportKind::Namespace);
    }

    #[test]
    fn parse_not_to_throw_is_weak() {
        let dir = write_temp(
            r#"
import { test, expect } from 'vitest';

test('not toThrow', () => {
    expect(() => doSomething()).not.toThrow();
});
"#,
            "not_throw.test.ts",
        );
        let path = dir.path().join("not_throw.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        let tb = &module.test_blocks[0];
        eprintln!("test_blocks.len()={}", module.test_blocks.len());
        eprintln!("assertion_count={}", tb.assertion_count);
        eprintln!("weak_assertion_count={}", tb.weak_assertion_count);
        eprintln!(
            "has_expect_call_without_assertion={}",
            tb.has_expect_call_without_assertion
        );

        assert_eq!(module.test_blocks.len(), 1);
        assert!(
            module.test_blocks[0].weak_assertion_count > 0,
            "not.toThrow() should be detected as weak assertion, weak_assertion_count={}",
            module.test_blocks[0].weak_assertion_count
        );
    }

    #[test]
    fn parse_detects_playwright_runtime_from_import() {
        let dir = write_temp(
            r#"
import { test, expect } from '@playwright/test';

test('pw test', async ({ page }) => {
    await expect(page).toHaveTitle(/app/);
});
"#,
            "pw.spec.ts",
        );
        let path = dir.path().join("pw.spec.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.runtime, TestRuntime::Playwright);
        assert!(module.playwright.is_some());
    }

    #[test]
    fn parse_detects_vitest_runtime() {
        let dir = write_temp(
            r#"
import { test, expect } from 'vitest';

test('vitest test', () => {
    expect(1).toBe(1);
});
"#,
            "vitest.test.ts",
        );
        let path = dir.path().join("vitest.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.runtime, TestRuntime::Vitest);
    }

    #[test]
    fn parse_detects_global_fetch_stub() {
        let dir = write_temp(
            r#"
import { vi, test } from 'vitest';

const mockFetch = vi.fn();
global.fetch = mockFetch;

test('test', () => {
    expect(1).toBe(1);
});
"#,
            "global_stub.test.ts",
        );
        let path = dir.path().join("global_stub.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert!(
            !module.global_stubs.is_empty(),
            "Expected global.fetch stub to be detected"
        );
        assert!(module.global_stubs.iter().any(|s| s.target == "fetch"));
    }

    #[test]
    fn parse_detects_vi_stub_global() {
        let dir = write_temp(
            r#"
import { vi, test } from 'vitest';

vi.stubGlobal('fetch', vi.fn(() => Promise.resolve({ json: () => ({}) })));
"#,
            "stub_global.test.ts",
        );
        let path = dir.path().join("stub_global.test.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert!(
            !module.global_stubs.is_empty(),
            "Expected vi.stubGlobal to be detected"
        );
        assert_eq!(module.global_stubs[0].target, "fetch");
    }

    #[test]
    fn parse_detects_axe_modules() {
        let dir = write_temp(
            r#"
import { test, expect } from '@playwright/test';
import { injectAxe, checkA11y } from 'axe-playwright';

test('a11y', async ({ page }) => {
    await injectAxe(page);
    await checkA11y(page);
});
"#,
            "a11y.spec.ts",
        );
        let path = dir.path().join("a11y.spec.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.runtime, TestRuntime::Playwright);
        let pw = module.playwright.as_ref().unwrap();
        assert!(
            pw.uses_axe,
            "Expected axe detection from axe-playwright import"
        );
    }

    #[test]
    fn parse_detects_playwright_wait_for_timeout() {
        let dir = write_temp(
            r#"
import { test, expect } from '@playwright/test';

test('with waitForTimeout', async ({ page }) => {
    await page.waitForTimeout(5000);
    await expect(page).toHaveTitle('app');
});
"#,
            "wait_for_timeout.spec.ts",
        );
        let path = dir.path().join("wait_for_timeout.spec.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        let pw = module.playwright.as_ref().unwrap();
        assert!(
            pw.calls
                .iter()
                .any(|c| c.call_name.contains("waitForTimeout")),
            "Expected waitForTimeout call to be tracked"
        );
    }

    #[test]
    fn parse_playwright_test_describe_only_creates_describe_block() {
        let dir = write_temp(
            r#"
import { test, expect } from '@playwright/test';

test.describe.only('focused group', () => {
    test('inside', async ({ page }) => {
        await expect(page).toHaveTitle(/app/);
    });
});
"#,
            "pw-describe-only.spec.ts",
        );
        let path = dir.path().join("pw-describe-only.spec.ts");
        let parser = TsParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.runtime, TestRuntime::Playwright);
        assert_eq!(
            module.describe_blocks.len(),
            1,
            "test.describe.only should create a DescribeBlock"
        );
        assert_eq!(module.describe_blocks[0].name, "focused group");
        assert!(
            module.describe_blocks[0].is_only,
            "Describe block should have is_only = true"
        );
        assert_eq!(
            module.test_blocks.len(),
            1,
            "Nested test should be parsed as TestBlock"
        );
        assert!(
            !module.test_blocks[0].is_only,
            "Nested test should NOT have is_only"
        );
    }
}
