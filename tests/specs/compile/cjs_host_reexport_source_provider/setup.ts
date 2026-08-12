await Deno.mkdir("allowed", { recursive: true });
await Promise.all([
  Deno.writeTextFile(
    "allowed/reexport.cjs",
    'module.exports = require("./inner.cjs");\n',
  ),
  Deno.writeTextFile("allowed/inner.cjs", 'exports.value = "from-inner";\n'),
  Deno.writeTextFile(
    "allowed/denied.cjs",
    'module.exports = require("../outside.cjs");\n',
  ),
  Deno.writeTextFile(
    "outside.cjs",
    "DENO_STANDALONE_CJS_SOURCE_CANARY = ;\n",
  ),
]);
