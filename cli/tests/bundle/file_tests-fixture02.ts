import * as b from "./subdir/b.ts";

console.log(b.b); // "b"
console.log(b.c); // { c: "c", default: class C }
