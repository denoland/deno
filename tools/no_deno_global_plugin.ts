// Copyright 2018-2025 the Deno authors. MIT license.

/**
 * This enforces using direct imports or primordials instead of
 * accessing the Deno namespace directly.
 */

const plugin: Deno.lint.Plugin = {
  name: "no-deno-global",
  rules: {
    "no-deno-global": {
      create(context) {
        return {
          'MemberExpression[object.name="Deno"]'(node) {
            context.report({
              node,
              message:
                "Do not use APIs via 'Deno.' global. Use direct imports or primordials instead.",
            });
          },
        };
      },
    },
  },
};

export default plugin;
