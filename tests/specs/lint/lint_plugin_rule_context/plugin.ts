import path from "node:path";

export default {
  name: "test-plugin",
  rules: {
    "my-rule": {
      create(context) {
        return {
          VariableDeclarator(node) {
            console.log(`ctx.id:`);
            console.log(context.id);
            console.log();

            console.log(`ctx.filename:`);
            console.log(path.relative(Deno.cwd(), context.filename));
            console.log();

            console.log(`ctx.getFilename():`);
            console.log(path.relative(Deno.cwd(), context.getFilename()));
            console.log();

            console.log(`ctx.getSourceCode():`);
            console.log(context.getSourceCode() === context.sourceCode);
            console.log();
          },
        };
      },
    },
  },
} satisfies Deno.lint.Plugin;
