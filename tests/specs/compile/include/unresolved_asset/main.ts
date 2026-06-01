// Regression test for https://github.com/denoland/deno/issues/27505
// `--include`ing a JS-like file with unresolvable imports must not fail the
// compilation; the file is embedded as an asset.
const asset = Deno.readTextFileSync(import.meta.dirname + "/asset.js");
console.log("asset embedded:", asset.includes('from "unknown"'));
