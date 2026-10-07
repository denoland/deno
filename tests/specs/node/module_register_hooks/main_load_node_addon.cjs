// Regression test for https://github.com/denoland/deno/issues/36240
// A registered `load` hook must not cause `require()` of a `.node` native
// addon to be compiled as CommonJS JavaScript. The file extension must still
// route to `Module._extensions[".node"]`, matching Node.js.
const { mkdtempSync, writeFileSync } = require("node:fs");
const { registerHooks } = require("node:module");
const { tmpdir } = require("node:os");
const { join } = require("node:path");

const dir = mkdtempSync(join(tmpdir(), "deno-36240-"));
const dummy = "\x00\x01\x02not-a-real-native-addon\x03\x04";
const before = join(dir, "before.node");
const after = join(dir, "after.node");
writeFileSync(before, dummy);
writeFileSync(after, dummy);

console.log("--- before registerHooks ---");
try {
  require(before);
} catch (e) {
  console.log(e.constructor.name);
}

registerHooks({
  resolve(specifier, context, nextResolve) {
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    return nextLoad(url, context);
  },
});

console.log("--- after registerHooks ---");
try {
  require(after);
} catch (e) {
  console.log(e.constructor.name);
}
