import { type ChildProcess, execFile } from "node:child_process";
import * as path from "node:path";
import * as vscode from "vscode";

interface Violation {
  rule_id: string;
  rule_name: string;
  severity: "Error" | "Warning" | "Info";
  category: string;
  message: string;
  file_path: string;
  line: number;
  col: number | null;
  suggestion: string | null;
  test_name: string | null;
}

const TEST_FILE_PATTERN = /\.(test|spec)\.[tj]sx?$/i;

const SEVERITY_MAP: Record<string, vscode.DiagnosticSeverity> = {
  Error: vscode.DiagnosticSeverity.Error,
  Warning: vscode.DiagnosticSeverity.Warning,
  Info: vscode.DiagnosticSeverity.Information,
};

let diagnosticCollection: vscode.DiagnosticCollection;
let lintTimeout: ReturnType<typeof setTimeout> | undefined;
const activeProcesses = new Map<string, ChildProcess>();

export function activate(context: vscode.ExtensionContext): void {
  diagnosticCollection =
    vscode.languages.createDiagnosticCollection("vitest-linter");

  context.subscriptions.push(diagnosticCollection);

  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (getConfig().run === "onSave") {
        lintDocument(doc);
      }
    }),
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeTextDocument((e) => {
      if (getConfig().run === "onType") {
        if (lintTimeout) {
          clearTimeout(lintTimeout);
        }
        lintTimeout = setTimeout(() => lintDocument(e.document), 300);
      }
    }),
  );

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((doc) => {
      if (getConfig().run === "onSave") {
        lintDocument(doc);
      }
    }),
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("vitest-linter")) {
        for (const doc of vscode.workspace.textDocuments) {
          lintDocument(doc);
        }
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "vitest-linter.lintWorkspace",
      lintWorkspace,
    ),
  );

  for (const doc of vscode.workspace.textDocuments) {
    lintDocument(doc);
  }
}

export function deactivate(): void {
  if (lintTimeout) {
    clearTimeout(lintTimeout);
  }
}

function getConfig() {
  const cfg = vscode.workspace.getConfiguration("vitest-linter");
  return {
    enable: cfg.get<boolean>("enable", true),
    executablePath: cfg.get<string>("executablePath", "vitest-linter"),
    run: cfg.get<"onSave" | "onType">("run", "onSave"),
    include: cfg.get<string[]>("include", []),
    exclude: cfg.get<string[]>("exclude", ["**/node_modules/**"]),
    severityOverrides: cfg.get<Record<string, string>>("severityOverrides", {}),
  };
}

function isTestFile(filePath: string): boolean {
  return TEST_FILE_PATTERN.test(filePath);
}

type SeverityOverride =
  | { kind: "off" }
  | { kind: "severity"; severity: vscode.DiagnosticSeverity };

function getSeverityOverride(
  ruleId: string,
  overrides: Record<string, string>,
): SeverityOverride | null {
  const override = overrides[ruleId];
  if (!override) {
    return null;
  }
  switch (override) {
    case "off":
      return { kind: "off" };
    case "info":
      return {
        kind: "severity",
        severity: vscode.DiagnosticSeverity.Information,
      };
    case "warning":
      return { kind: "severity", severity: vscode.DiagnosticSeverity.Warning };
    case "error":
      return { kind: "severity", severity: vscode.DiagnosticSeverity.Error };
    default:
      return null;
  }
}

function lintDocument(doc: vscode.TextDocument): void {
  const config = getConfig();
  if (!config.enable) {
    diagnosticCollection.delete(doc.uri);
    return;
  }

  if (doc.uri.scheme !== "file") {
    return;
  }

  const filePath = doc.uri.fsPath;

  if (!isTestFile(filePath)) {
    return;
  }

  const workspaceFolder = vscode.workspace.getWorkspaceFolder(doc.uri);
  const cwd = workspaceFolder
    ? workspaceFolder.uri.fsPath
    : path.dirname(filePath);

  const relPath = path.relative(cwd, filePath);
  for (const pattern of config.exclude) {
    if (matchGlob(relPath, pattern)) {
      diagnosticCollection.delete(doc.uri);
      return;
    }
  }

  if (config.include.length > 0) {
    const included = config.include.some((pattern) =>
      matchGlob(relPath, pattern),
    );
    if (!included) {
      diagnosticCollection.delete(doc.uri);
      return;
    }
  }

  const args = ["--format", "json", "--no-color", filePath];

  const existing = activeProcesses.get(filePath);
  if (existing) {
    existing.kill();
  }

  const proc = execFile(
    config.executablePath,
    args,
    { cwd, timeout: 30_000, maxBuffer: 10 * 1024 * 1024 },
    (err, stdout) => {
      activeProcesses.delete(filePath);
      if (err && err.code !== 1 && !stdout) {
        if ((err as NodeJS.ErrnoException).code === "ENOENT") {
          void vscode.window.showErrorMessage(
            `vitest-linter: executable not found at "${config.executablePath}". Install it or set vitest-linter.executablePath.`,
          );
        } else {
          const msg = `vitest-linter: ${err.message}`;
          void vscode.window.setStatusBarMessage(msg, 5000);
        }
        return;
      }

      let violations: Violation[];
      try {
        violations = JSON.parse(stdout || "[]") as Violation[];
      } catch {
        return;
      }

      const diagnostics: vscode.Diagnostic[] = [];

      for (const v of violations) {
        if (
          v.file_path !== filePath &&
          path.resolve(cwd, v.file_path) !== filePath
        ) {
          continue;
        }

        const severityOverride = getSeverityOverride(
          v.rule_id,
          config.severityOverrides,
        );
        if (severityOverride?.kind === "off") {
          continue;
        }

        const severity =
          (severityOverride?.kind === "severity"
            ? severityOverride.severity
            : undefined) ??
          SEVERITY_MAP[v.severity] ??
          vscode.DiagnosticSeverity.Warning;
        const line = Math.max(0, v.line - 1);
        const col = v.col ? Math.max(0, v.col - 1) : 0;

        const range = new vscode.Range(line, col, line, col + 1);

        let message = `${v.rule_id}: ${v.message}`;
        if (v.suggestion) {
          message += `\nSuggestion: ${v.suggestion}`;
        }

        const diag = new vscode.Diagnostic(range, message, severity);
        diag.source = "vitest-linter";
        diag.code = v.rule_id;
        diagnostics.push(diag);
      }

      diagnosticCollection.set(doc.uri, diagnostics);
    },
  );
  activeProcesses.set(filePath, proc);
}

function lintWorkspace(): void {
  const config = getConfig();
  if (!config.enable) {
    return;
  }

  const folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    return;
  }

  diagnosticCollection.clear();

  let totalCount = 0;
  let totalErrors = 0;
  let pending = folders.length;

  for (const folder of folders) {
    const cwd = folder.uri.fsPath;
    const args = ["--format", "json", "--no-color", "."];

    execFile(
      config.executablePath,
      args,
      { cwd, timeout: 60_000, maxBuffer: 50 * 1024 * 1024 },
      (err, stdout) => {
        pending--;
        if (err && err.code !== 1 && !stdout) {
          void vscode.window.showErrorMessage(
            `vitest-linter workspace lint failed: ${err.message}`,
          );
          if (pending === 0) {
            void vscode.window.showInformationMessage(
              `Vitest Linter: ${totalCount} violation(s) (${totalErrors} error${totalErrors !== 1 ? "s" : ""})`,
            );
          }
          return;
        }

        let violations: Violation[];
        try {
          violations = JSON.parse(stdout || "[]") as Violation[];
        } catch {
          if (pending === 0) {
            void vscode.window.showInformationMessage(
              `Vitest Linter: ${totalCount} violation(s) (${totalErrors} error${totalErrors !== 1 ? "s" : ""})`,
            );
          }
          return;
        }

        totalCount += violations.length;
        totalErrors += violations.filter((v) => v.severity === "Error").length;

        const byFile = new Map<string, Violation[]>();
        for (const v of violations) {
          const absPath = path.resolve(cwd, v.file_path);
          const relPath = path.relative(cwd, absPath);

          let excluded = false;
          for (const pattern of config.exclude) {
            if (matchGlob(relPath, pattern)) {
              excluded = true;
              break;
            }
          }
          if (excluded) continue;

          if (config.include.length > 0) {
            const included = config.include.some((p) => matchGlob(relPath, p));
            if (!included) continue;
          }

          const existing = byFile.get(absPath);
          if (existing) {
            existing.push(v);
          } else {
            byFile.set(absPath, [v]);
          }
        }

        for (const [absPath, fileViolations] of byFile) {
          const uri = vscode.Uri.file(absPath);
          const diagnostics: vscode.Diagnostic[] = [];

          for (const v of fileViolations) {
            const severityOverride = getSeverityOverride(
              v.rule_id,
              config.severityOverrides,
            );
            if (severityOverride?.kind === "off") {
              continue;
            }

            const severity =
              (severityOverride?.kind === "severity"
                ? severityOverride.severity
                : undefined) ??
              SEVERITY_MAP[v.severity] ??
              vscode.DiagnosticSeverity.Warning;
            const line = Math.max(0, v.line - 1);
            const col = v.col ? Math.max(0, v.col - 1) : 0;

            const range = new vscode.Range(line, col, line, col + 1);

            let message = `${v.rule_id}: ${v.message}`;
            if (v.suggestion) {
              message += `\nSuggestion: ${v.suggestion}`;
            }

            const diag = new vscode.Diagnostic(range, message, severity);
            diag.source = "vitest-linter";
            diag.code = v.rule_id;
            diagnostics.push(diag);
          }

          diagnosticCollection.set(uri, diagnostics);
        }

        if (pending === 0) {
          void vscode.window.showInformationMessage(
            `Vitest Linter: ${totalCount} violation(s) (${totalErrors} error${totalErrors !== 1 ? "s" : ""})`,
          );
        }
      },
    );
  }
}

function matchGlob(filePath: string, pattern: string): boolean {
  const regexStr = pattern
    .replace(/[.+^${}()|[\]\\]/g, "\\$&")
    .replace(/\*\*/g, "{{GLOBSTAR}}")
    .replace(/\*/g, "[^/]*")
    .replace(/\?/g, "[^/]")
    .replace(/\{\{GLOBSTAR\}\}/g, ".*");
  try {
    return new RegExp(`(^|/)${regexStr}$`).test(filePath);
  } catch {
    return false;
  }
}
