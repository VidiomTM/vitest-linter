const { getViolations, clearCache } = require("./lib/runner");
const ruleDefinitions = require("./lib/rules");

const rules = {};
const recommendedRules = {};

for (const def of ruleDefinitions) {
  rules[def.eslintName] = {
    meta: {
      type: "problem",
      docs: {
        description: def.description,
        category: "Test Smells",
        recommended: true,
        url: `https://github.com/VidiomTM/vitest-linter#rule-${def.ruleId.toLowerCase()}`,
      },
      schema: [],
    },
    create(context) {
      const filePath = context.filename ?? context.getFilename();
      return {
        Program() {
          const violations = getViolations(filePath);
          const matched = violations.filter((v) => v.rule_id === def.ruleId);
          for (const v of matched) {
            context.report({
              loc: {
                start: { line: v.line, column: (v.col ?? 1) - 1 },
                end: { line: v.line, column: (v.col ?? 1) - 1 + 1 },
              },
              message: `[${v.rule_id}] ${v.message}${v.suggestion ? ` Suggestion: ${v.suggestion}` : ""}`,
            });
          }
        },
      };
    },
  };

  recommendedRules[`vitest-linter/${def.eslintName}`] = "warn";
}

const plugin = {
  meta: {
    name: "eslint-plugin-vitest-linter",
    version: "0.1.0",
  },
  rules,
  configs: {},
  processors: {},
  // Expose cache invalidation so consumers (e.g. watch mode) can reset
  // the per-file violation cache between runs.
  clearCache,
};

plugin.configs.recommended = {
  plugins: ["vitest-linter"],
  rules: recommendedRules,
};

plugin.configs["flat/recommended"] = {
  name: "vitest-linter/recommended",
  plugins: {
    "vitest-linter": plugin,
  },
  rules: recommendedRules,
};

module.exports = plugin;
