// Exercises the default read confinement's per-imported-package grant. Loading
// this module grants read on this package's own folder in the global npm cache,
// so reading a bundled data file here succeeds. A different package in the same
// cache is not covered by that grant, so reading its file is denied.
const path = require("node:path");

// Read our own bundled data file: allowed by the per-package grant.
const own = Deno.readTextFileSync(path.join(__dirname, "data.txt"));
console.log("read own package file:", own.trim());

// Read a different cached package's file: same npm cache, different folder, so
// the per-package grant does not cover it.
const otherFile = path.join(
  __dirname,
  "..",
  "..",
  "read-scope-other",
  "1.0.0",
  "secret.txt",
);
try {
  Deno.readTextFileSync(otherFile);
  console.log("read other package file: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-read") ? "--allow-read" : "?";
  console.log(`read other package file: ${err.name} ${named}`);
}
