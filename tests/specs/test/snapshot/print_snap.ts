const content = Deno.readTextFileSync(
  new URL("__snapshots__/mismatch_test.ts.snap", import.meta.url),
);
console.log("has stale entry:", content.includes("stale entry"));
console.log("has updated value:", content.includes("value: 2"));
