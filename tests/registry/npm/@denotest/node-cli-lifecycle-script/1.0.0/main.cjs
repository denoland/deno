// Run by `node --require ./helper.cjs main.cjs`. Writes a marker recording
// whether the `--require` preload ran ("object" when run under Deno, "undefined"
// if the `--require` flag was dropped during translation).
const fs = require("fs");
const path = require("path");
fs.writeFileSync(
  path.join(process.env.INIT_CWD, "node_require_ran.txt"),
  "require " + globalThis.__node_require_preload,
);
