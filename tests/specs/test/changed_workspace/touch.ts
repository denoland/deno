// Modify a source file in workspace member `b`. Both `b/b_test.ts` and the test
// in member `a` that imports it should be selected, since selection spans the
// whole workspace's single module graph.
Deno.writeTextFileSync(
  "b/mod.ts",
  Deno.readTextFileSync("b/mod.ts") + "\n// changed\n",
);
