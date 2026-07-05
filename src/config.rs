use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use globset::{Glob, GlobMatcher};
use serde::Deserialize;

const CONFIG_FILE_NAME: &str = ".vitest-linter.toml";
const DEFAULT_INTEGRATION_GLOB: &str = "**/*.integration.test.{ts,tsx,js,jsx}";

#[derive(Debug, Deserialize, Default)]
struct RawConfig {
    #[serde(default)]
    deps: RawDepsConfig,
    #[serde(default)]
    rules: RawRulesConfig,
}

#[derive(Debug, Deserialize, Default)]
struct RawRulesConfig {
    /// Per-rule severity overrides: {"VITEST-FLK-001": "off", "VITEST-MNT-001": "warning"}
    #[serde(default)]
    select: HashMap<String, String>,
    #[serde(rename = "VITEST-DEP-001")]
    dep001: Option<RawDep001RuleConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct RawDep001RuleConfig {
    #[serde(default)]
    stable_path_segments: Vec<String>,
    #[serde(default)]
    stable_file_suffixes: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RawDepsConfig {
    #[serde(default)]
    banned_mock_paths: Vec<String>,
    #[serde(default)]
    banned_singletons: Vec<RawBannedSingleton>,
    #[serde(default)]
    integration_test_glob: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RawBannedSingleton {
    from: String,
    #[serde(default)]
    names: Vec<String>,
}

/// Parsed configuration from `.vitest-linter.toml` and `package.json`.
#[derive(Debug)]
pub struct Config {
    pub deps: DepsConfig,
    pub rules: RulesConfig,
    /// Cached `tsconfig.json` for path-alias resolution, loaded once at
    /// construction time from the config root directory.
    tsconfig: Option<TsConfig>,
}

/// Per-rule severity overrides and enable/disable settings.
#[derive(Debug, Default)]
pub struct RulesConfig {
    /// Per-rule severity overrides. Key = rule ID, value = "off" | "info" | "warning" | "error"
    pub select: HashMap<String, String>,
    pub dep001: Dep001RuleConfig,
}

#[derive(Debug)]
pub struct Dep001RuleConfig {
    pub stable_path_segments: Vec<String>,
    pub stable_file_suffixes: Vec<String>,
}

impl RulesConfig {
    /// Returns `true` if the given rule is turned off.
    #[must_use]
    pub fn is_disabled(&self, rule_id: &str) -> bool {
        self.select
            .get(rule_id)
            .is_some_and(|v| v.eq_ignore_ascii_case("off"))
    }

    /// Returns an overridden severity string for the rule, if any.
    #[must_use]
    pub fn severity_override(&self, rule_id: &str) -> Option<&str> {
        self.select.get(rule_id).map(|s| s.as_str())
    }
}

impl Default for Dep001RuleConfig {
    fn default() -> Self {
        Self {
            stable_path_segments: vec![
                "utils".to_string(),
                "helpers".to_string(),
                "mappers".to_string(),
                "transforms".to_string(),
                "validators".to_string(),
                "schemas".to_string(),
            ],
            stable_file_suffixes: vec![
                "-utils".to_string(),
                "-mapper".to_string(),
                "-helpers".to_string(),
                "-schema".to_string(),
            ],
        }
    }
}

#[derive(Debug, Default)]
pub struct DepsConfig {
    pub banned_mock_paths: Vec<GlobMatcher>,
    pub banned_singletons: Vec<BannedSingleton>,
    pub integration_test_glob: Option<GlobMatcher>,
}

#[derive(Debug)]
pub struct BannedSingleton {
    pub from: GlobMatcher,
    pub names: Vec<String>,
}

impl Config {
    /// Load `.vitest-linter.toml` by walking up from `start` until found, or
    /// return an empty config when no file exists. Also checks `package.json`
    /// for a `vitest-linter` key and merges it.
    pub fn load_from(start: &Path) -> Result<Self> {
        let (mut config, config_root) = if let Some(found) = find_config(start) {
            let root = found
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| start.to_path_buf());
            (Self::from_path(&found)?, root)
        } else {
            (Self::default(), start.to_path_buf())
        };

        config.tsconfig = TsConfig::load_from(&config_root);
        merge_package_json_overrides(&mut config, start);

        Ok(config)
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        let raw_text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config {}", path.display()))?;
        Self::parse_toml(&raw_text)
    }

    pub fn parse_toml(text: &str) -> Result<Self> {
        let raw: RawConfig =
            toml::from_str(text).with_context(|| "parsing vitest-linter config")?;
        Self::from_raw(raw)
    }

    fn from_raw(raw: RawConfig) -> Result<Self> {
        let mut banned_mock_paths = Vec::new();
        for pat in &raw.deps.banned_mock_paths {
            banned_mock_paths.push(compile_glob(pat)?);
        }
        let mut banned_singletons = Vec::new();
        for entry in raw.deps.banned_singletons {
            banned_singletons.push(BannedSingleton {
                from: compile_glob(&entry.from)?,
                names: entry.names,
            });
        }
        let integration_test_glob = match raw.deps.integration_test_glob.as_deref() {
            Some(p) => Some(compile_glob(p)?),
            None => Some(compile_glob(DEFAULT_INTEGRATION_GLOB)?),
        };
        Ok(Self {
            deps: DepsConfig {
                banned_mock_paths,
                banned_singletons,
                integration_test_glob,
            },
            rules: RulesConfig {
                select: raw.rules.select,
                dep001: raw
                    .rules
                    .dep001
                    .map(|cfg| Dep001RuleConfig {
                        stable_path_segments: if cfg.stable_path_segments.is_empty() {
                            Dep001RuleConfig::default().stable_path_segments
                        } else {
                            cfg.stable_path_segments
                        },
                        stable_file_suffixes: if cfg.stable_file_suffixes.is_empty() {
                            Dep001RuleConfig::default().stable_file_suffixes
                        } else {
                            cfg.stable_file_suffixes
                        },
                    })
                    .unwrap_or_default(),
            },
            tsconfig: None,
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        let integration_test_glob = compile_glob(DEFAULT_INTEGRATION_GLOB).ok();
        Self {
            deps: DepsConfig {
                banned_mock_paths: Vec::new(),
                banned_singletons: Vec::new(),
                integration_test_glob,
            },
            rules: RulesConfig::default(),
            tsconfig: None,
        }
    }
}

impl Config {
    /// Resolve a module import path to an absolute path.
    /// Returns the path as-is if it cannot be resolved.
    #[must_use]
    pub fn resolve_module_path(&self, import_path: &str) -> String {
        // Use the tsconfig cached at construction time if present.
        if let Some(tsconfig) = &self.tsconfig {
            if let Some(resolved) = tsconfig.resolve(import_path, Path::new(".")) {
                return resolved.to_string_lossy().to_string();
            }
        }
        import_path.to_string()
    }
}

fn compile_glob(pat: &str) -> Result<GlobMatcher> {
    Ok(Glob::new(pat)
        .with_context(|| format!("compiling glob `{pat}`"))?
        .compile_matcher())
}

/// Parsed TypeScript configuration for path alias resolution.
#[derive(Debug)]
pub struct TsConfig {
    base_url: PathBuf,
    /// (glob_matcher, targets, alias_prefix) — alias_prefix is the part before `/*`
    paths: Vec<(GlobMatcher, Vec<String>, String)>,
}

impl TsConfig {
    /// Load `tsconfig.json` from the given project root directory.
    #[must_use]
    pub fn load_from(project_root: &Path) -> Option<Self> {
        let tsconfig_path = project_root.join("tsconfig.json");
        let text = std::fs::read_to_string(&tsconfig_path).ok()?;
        let raw: serde_json::Value = serde_json::from_str(&text).ok()?;

        let compiler_options = raw.get("compilerOptions")?;
        let base_url = compiler_options
            .get("baseUrl")
            .and_then(|v| v.as_str())
            .map(|b| project_root.join(b))
            .unwrap_or_else(|| project_root.to_path_buf());

        let mut paths = Vec::new();
        if let Some(paths_obj) = compiler_options.get("paths").and_then(|v| v.as_object()) {
            for (alias, targets) in paths_obj {
                if let Some(arr) = targets.as_array() {
                    let target_strs: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                    // Convert TypeScript path pattern to glob: @/* → @/**
                    let glob_pattern = alias.replace("/*", "/**");
                    if let Ok(glob) = Glob::new(&glob_pattern).map(|g| g.compile_matcher()) {
                        // Store the alias prefix (part before /*) for substitution
                        let alias_prefix = alias.strip_suffix("/*").unwrap_or(alias).to_string();
                        paths.push((glob, target_strs, alias_prefix));
                    }
                }
            }
        }

        Some(Self { base_url, paths })
    }

    /// Resolve an import path using the tsconfig path aliases.
    #[must_use]
    pub fn resolve(&self, import_path: &str, _from_dir: &Path) -> Option<PathBuf> {
        for (matcher, targets, alias_prefix) in &self.paths {
            if matcher.is_match(import_path) {
                // Extract the matched portion by stripping the alias prefix
                let matched_part = import_path
                    .strip_prefix(alias_prefix)
                    .and_then(|s| s.strip_prefix('/'))
                    .unwrap_or(import_path);
                for target in targets {
                    // Substitute the matched portion into the target pattern
                    let resolved = if let Some(target_prefix) = target.strip_suffix("/*") {
                        format!("{}/{}", target_prefix, matched_part)
                    } else {
                        target.clone()
                    };
                    let full_path = self.base_url.join(&resolved);
                    if let Some(found) = try_with_extensions(&full_path) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }
}

fn try_with_extensions(path: &Path) -> Option<PathBuf> {
    let exts = [".ts", ".tsx", ".js", ".jsx"];
    for ext in &exts {
        let with_ext = path.with_extension(ext.strip_prefix('.').unwrap());
        if with_ext.is_file() {
            return Some(with_ext);
        }
    }
    let index_names = ["index.ts", "index.tsx", "index.js", "index.jsx"];
    for name in &index_names {
        let idx = path.join(name);
        if idx.is_file() {
            return Some(idx);
        }
    }
    None
}

fn find_config(start: &Path) -> Option<PathBuf> {
    let mut cur = if start.is_dir() {
        Some(start.to_path_buf())
    } else {
        start.parent().map(Path::to_path_buf)
    };
    while let Some(dir) = cur {
        let candidate = dir.join(CONFIG_FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        cur = dir.parent().map(Path::to_path_buf);
    }
    None
}

fn find_package_json_dir(start: &Path) -> Option<PathBuf> {
    let mut cur = if start.is_dir() {
        Some(start.to_path_buf())
    } else {
        start.parent().map(Path::to_path_buf)
    };
    while let Some(dir) = cur {
        let candidate = dir.join("package.json");
        if candidate.is_file() {
            return Some(dir);
        }
        cur = dir.parent().map(Path::to_path_buf);
    }
    None
}

/// Merge `vitest-linter` config from `package.json` into the config struct.
/// Uses early returns to keep cognitive complexity low.
fn merge_package_json_overrides(config: &mut Config, start: &Path) {
    let pkg_dir = match find_package_json_dir(start) {
        Some(d) => d,
        None => return,
    };
    let pkg_path = pkg_dir.join("package.json");
    let pkg_text = match std::fs::read_to_string(&pkg_path) {
        Ok(t) => t,
        Err(_) => return,
    };
    let pkg = match serde_json::from_str::<serde_json::Value>(&pkg_text) {
        Ok(v) => v,
        Err(_) => return,
    };
    let Some(vl_config) = pkg.get("vitest-linter") else {
        return;
    };
    let Some(select) = vl_config.get("select").and_then(|s| s.as_object()) else {
        return;
    };
    for (key, val) in select {
        if let Some(severity) = val.as_str() {
            config
                .rules
                .select
                .insert(key.clone(), severity.to_string());
        }
    }
}

/// Match a `vi.mock(<source>)` source string against the banlist. The check
/// matches both the literal pattern and a normalized form without leading `./`
/// or `../` segments to keep config simple for callers.
#[must_use]
pub fn matches_path(matchers: &[GlobMatcher], source: &str) -> bool {
    if matchers.is_empty() {
        return false;
    }
    let normalized = source.trim_start_matches("./");
    let stripped = trim_relative_prefix(normalized);
    matchers
        .iter()
        .any(|g| g.is_match(source) || g.is_match(normalized) || g.is_match(stripped))
}

fn trim_relative_prefix(s: &str) -> &str {
    let mut s = s;
    loop {
        if let Some(rest) = s.strip_prefix("../") {
            s = rest;
        } else {
            return s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_default_integration_glob() {
        let cfg = Config::default();
        assert!(cfg.deps.integration_test_glob.is_some());
        let g = cfg.deps.integration_test_glob.unwrap();
        assert!(g.is_match("foo/bar.integration.test.ts"));
        assert!(!g.is_match("foo/bar.test.ts"));
    }

    #[test]
    fn parse_full_config() {
        let text = r#"
[deps]
banned_mock_paths = [
  "**/infrastructure/database",
  "**/infrastructure/event-bus",
]
integration_test_glob = "**/*.integration.test.ts"

[[deps.banned_singletons]]
from = "**/services/pipeline-orchestrator"
names = ["orchestrator"]

[[deps.banned_singletons]]
from = "**/services/progress-persistence"
names = ["progressPersistence"]
"#;
        let cfg = Config::parse_toml(text).unwrap();
        assert_eq!(cfg.deps.banned_mock_paths.len(), 2);
        assert_eq!(cfg.deps.banned_singletons.len(), 2);
        assert!(matches_path(
            &cfg.deps.banned_mock_paths,
            "../infrastructure/database",
        ));
        assert!(matches_path(
            &cfg.deps.banned_mock_paths,
            "src/lib/infrastructure/event-bus",
        ));
        assert!(!matches_path(
            &cfg.deps.banned_mock_paths,
            "../infrastructure/audio",
        ));
    }

    #[test]
    fn singleton_glob_match() {
        let text = r#"
[[deps.banned_singletons]]
from = "**/services/pipeline-orchestrator"
names = ["orchestrator"]
"#;
        let cfg = Config::parse_toml(text).unwrap();
        let s = &cfg.deps.banned_singletons[0];
        // Glob requires `services/` segment present somewhere in the path.
        assert!(s
            .from
            .is_match("apps/platform/src/lib/server/services/pipeline-orchestrator"));
        assert!(!s.from.is_match("./pipeline-orchestrator"));
    }

    #[test]
    fn find_config_walks_up() {
        let dir = tempfile::TempDir::new().unwrap();
        let nested = dir.path().join("a/b/c");
        std::fs::create_dir_all(&nested).unwrap();
        let cfg_path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&cfg_path, "").unwrap();
        let found = find_config(&nested).unwrap();
        assert_eq!(found, cfg_path);
    }

    #[test]
    fn tsconfig_resolves_paths_alias() {
        let dir = tempfile::TempDir::new().unwrap();
        let tsconfig = r#"{
            "compilerOptions": {
                "baseUrl": ".",
                "paths": {
                    "@/*": ["src/*"],
                    "@utils/*": ["lib/utils/*"]
                }
            }
        }"#;
        std::fs::write(dir.path().join("tsconfig.json"), tsconfig).unwrap();

        let button_dir = dir.path().join("src/components");
        std::fs::create_dir_all(&button_dir).unwrap();
        std::fs::write(button_dir.join("Button.ts"), "export default {};").unwrap();

        let ts = TsConfig::load_from(dir.path()).unwrap();
        let resolved = ts.resolve("@/components/Button", dir.path());
        assert!(resolved.is_some());
        let resolved = resolved.unwrap();
        assert!(resolved.ends_with("src/components/Button.ts"));
    }

    #[test]
    fn tsconfig_returns_none_for_unmatched_alias() {
        let dir = tempfile::TempDir::new().unwrap();
        let tsconfig = r#"{
            "compilerOptions": {
                "baseUrl": ".",
                "paths": {
                    "@/*": ["src/*"]
                }
            }
        }"#;
        std::fs::write(dir.path().join("tsconfig.json"), tsconfig).unwrap();

        let ts = TsConfig::load_from(dir.path()).unwrap();
        let resolved = ts.resolve("lodash", dir.path());
        assert!(resolved.is_none());
    }

    #[test]
    fn tsconfig_returns_none_when_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let ts = TsConfig::load_from(dir.path());
        assert!(ts.is_none());
    }

    #[test]
    fn tsconfig_specific_alias_takes_priority_over_wildcard() {
        // With both `@/*` and `@utils/*` aliases, importing `@utils/foo` must
        // resolve via the more specific `@utils/*` alias (lib/utils/foo), not
        // the broader `@/*` alias (src/utils/foo). A naive glob match on `@/**`
        // would also match `@utils/foo`, so resolution must prefer the longest
        // matching alias prefix.
        let dir = tempfile::TempDir::new().unwrap();
        let tsconfig = r#"{
            "compilerOptions": {
                "baseUrl": ".",
                "paths": {
                    "@/*": ["src/*"],
                    "@utils/*": ["lib/utils/*"]
                }
            }
        }"#;
        std::fs::write(dir.path().join("tsconfig.json"), tsconfig).unwrap();

        let utils_dir = dir.path().join("lib/utils");
        std::fs::create_dir_all(&utils_dir).unwrap();
        std::fs::write(utils_dir.join("foo.ts"), "export const x = 1;").unwrap();

        let src_utils_dir = dir.path().join("src/utils");
        std::fs::create_dir_all(&src_utils_dir).unwrap();
        std::fs::write(src_utils_dir.join("foo.ts"), "export const WRONG = 1;").unwrap();

        let ts = TsConfig::load_from(dir.path()).unwrap();
        let resolved = ts.resolve("@utils/foo", dir.path());
        assert!(resolved.is_some(), "expected @utils/foo to resolve");
        let resolved = resolved.unwrap();
        assert!(
            resolved.ends_with("lib/utils/foo.ts"),
            "expected specific alias @utils/* to win, got {}",
            resolved.display()
        );
    }

    #[test]
    fn config_resolve_module_path_uses_cached_tsconfig() {
        // `Config::load_from` must cache the tsconfig found at the config root
        // and `resolve_module_path` must use that cache rather than re-reading
        // from disk (e.g. from the process cwd) on every call.
        //
        // We build a config rooted at `dir` (which has a tsconfig with a
        // `@/*` alias) and then assert resolution works even though the
        // process cwd has no tsconfig. A regression that re-reads tsconfig
        // from `Path::new(".")` would return `None` and fail to resolve.
        let dir = tempfile::TempDir::new().unwrap();
        let tsconfig = r#"{
            "compilerOptions": {
                "baseUrl": ".",
                "paths": {
                    "@/*": ["src/*"]
                }
            }
        }"#;
        std::fs::write(dir.path().join("tsconfig.json"), tsconfig).unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE_NAME), "").unwrap();

        let src_dir = dir.path().join("src/components");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("Button.ts"), "export default {};").unwrap();

        let config = Config::load_from(dir.path()).unwrap();
        let resolved = config.resolve_module_path("@/components/Button");
        assert!(
            resolved.ends_with("src/components/Button.ts"),
            "expected cached tsconfig to resolve @/components/Button, got {resolved}"
        );
    }
}
