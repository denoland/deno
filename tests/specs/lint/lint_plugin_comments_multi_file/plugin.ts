export default {
  name: "comments-test",
  rules: {
    "report-comments": {
      create(ctx) {
        return {
          Program(node) {
            const comments = node.comments;
            const values = comments.map((
              c: Deno.lint.LineComment | Deno.lint.BlockComment,
            ) => c.value.trim());
            console.log(`${ctx.filename}: ${JSON.stringify(values)}`);
          },
        };
      },
    },
  },
} satisfies Deno.lint.Plugin;
