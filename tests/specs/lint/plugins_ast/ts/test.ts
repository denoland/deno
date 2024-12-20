// TODO(@marvinhagemeister) Remove once we land "official" types
interface LintContext {
  report(node: unknown): void;
}

// TODO(@marvinhagemeister) Remove once we land "official" types
interface LintRule {
  create(ctx: LintContext): Record<string, (node: unknown) => void>;
  destroy?(): void;
}

// TODO(@marvinhagemeister) Remove once we land "official" types
interface LintPlugin {
  name: string;
  rules: Record<string, LintRule>;
}

function runLintPlugin(plugin: LintPlugin, fileName: string, source: string) {
  // deno-lint-ignore no-explicit-any
  return (Deno as any)[(Deno as any).internal].runLintPlugin(
    plugin,
    fileName,
    source,
  );
}

function testPlugin(
  fileName: string,
  source: string,
  rule: LintRule,
) {
  const plugin = {
    name: "test-plugin",
    rules: {
      testRule: rule,
    },
  };

  return runLintPlugin(plugin, fileName, source);
}

Deno.test.only("Plugin - MemberExpression", () => {
  const res = testPlugin("source.ts", "foo.bar", {
    create(ctx) {
      return {
        MemberExpression(node) {
          ctx.report({
            node,
            message: "found",
          });
        },
      };
    },
  });
  console.log(res);
});

Deno.test("Plugin - CallExpression", () => {
  testPlugin("source.ts", "foo()", {
    create(ctx) {
      return {
        CallExpression(node) {
          console.log(node);
        },
      };
    },
  });
});
