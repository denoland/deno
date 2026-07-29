// deno-lint-ignore-file no-console
console.log("Worker: bundling module");
const allowed = await Deno.bundle({
  entrypoints: ["./worker.ts"],
  write: false,
});
console.log("Worker: allowed bundle result.success:", allowed.success);

// Only `./worker.ts` is readable, so bundling `./main.ts` is denied: the
// bundle reads (and would return) module source, which requires read access.
let readDenied = false;
try {
  const denied = await Deno.bundle({
    entrypoints: ["./main.ts"],
    write: false,
  });
  readDenied = !denied.success &&
    denied.errors.some((error) => error.text.includes("Requires read access"));
} catch (err) {
  readDenied = String(err).includes("Requires read access");
}
console.log("Worker: read denied:", readDenied);

postMessage("done");
