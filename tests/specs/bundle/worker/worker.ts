// deno-lint-ignore-file no-console
console.log("Worker: bundling module");
// Only `./worker.ts` is readable, so bundling `./main.ts` is denied: the
// bundle reads (and would return) module source, which requires read access.
const result = await Deno.bundle({
  entrypoints: ["./main.ts"],
  write: false,
  outputDir: "/",
});
console.log("Worker: bundle result.success:", result.success);
console.log(
  "Worker: read denied:",
  result.errors.some((e) => e.text.includes("Requires read access")),
);

postMessage("done");
