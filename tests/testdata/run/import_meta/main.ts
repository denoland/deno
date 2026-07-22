import { assertThrows } from "@std/assert/throws";
import "http://localhost:4545/run/import_meta/other.ts";
import "./other.ts";

console.log(
  "main",
  import.meta.url,
  import.meta.main,
  import.meta.filename,
  import.meta.dirname,
);

console.log("Resolving ./foo.js", import.meta.resolve("./foo.js"));
console.log("Resolving bare from import map", import.meta.resolve("bare"));
console.log(
  "Resolving https://example.com/rewrite from import map",
  import.meta.resolve("https://example.com/rewrite"),
);
console.log(
  "Resolving without a value from import map",
  import.meta.resolve(),
);
console.log(
  "Resolving 1 from import map",
  import.meta.resolve(1),
);
console.log(
  "Resolving null from import map",
  import.meta.resolve(null),
);
console.log(
  "Resolving object from import map",
  import.meta.resolve({}),
);
assertThrows(() => {
  import.meta.resolve("too", "many", "arguments");
}, TypeError);
assertThrows(() => {
  import.meta.resolve("://malformed/url?asdf");
}, TypeError);
console.log(
  "Resolving npm:cowsay",
  import.meta.resolve("npm:cowsay"),
);
console.log(
  "Resolving npm:cowsay@1",
  import.meta.resolve("npm:cowsay@1"),
);
console.log(
  "Resolving npm:preact from import map",
  import.meta.resolve("npm:preact"),
);
