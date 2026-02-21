// deno-lint-ignore-file no-console
console.log("Worker: bundling module");
const result = await Deno.bundle({
  entrypoints: ["./main.ts"],
  write: false,
  outputDir: "/",
});
console.log("Worker: bundle result.success:", result.success);

postMessage("done");
