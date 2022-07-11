console.log("import_meta", import.meta.url, import.meta.main);

import "./import_meta2.ts";

console.log("Resolving ./foo.js", import.meta.resolve("./foo.js"));
console.log("Resolving bare from import map", import.meta.resolve("bare"));
console.log(
  "Resolving https://example.com/rewrite from import map",
  import.meta.resolve("https://example.com/rewrite"),
);
console.log(
  "Resolving https://example.com/rewrite from import map",
  import.meta.resolve("https://example.com/rewrite"),
);
console.log(
  "Resolving without a value from import map",
  import.meta.resolve(),
);
console.log(
  "Resolving undefined from import map",
  import.meta.resolve("https://example.com/PASS-undefined"),
);
console.log(
  "Resolving 1 from import map",
  import.meta.resolve("https://example.com/PASS-1"),
);
console.log(
  "Resolving null from import map",
  import.meta.resolve("https://example.com/PASS-null"),
);
