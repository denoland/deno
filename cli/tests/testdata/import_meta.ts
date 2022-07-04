console.log("import_meta", import.meta.url, import.meta.main);

import "./import_meta2.ts";

console.log("Resolving ./foo.js", import.meta.resolve("./foo.js"));
console.log(
  "Resolving ./foo.js from ./bar.js",
  import.meta.resolve("./foo.js", "./bar.js"),
);
