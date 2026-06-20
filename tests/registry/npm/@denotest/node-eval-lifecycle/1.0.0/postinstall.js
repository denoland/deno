const fs = require("fs");
const path = require("path");
const pkg = require("./package.json");

fs.writeFileSync(
  path.join(process.env.INIT_CWD, "node-eval-lifecycle.txt"),
  `installed ${pkg.name}`,
);
