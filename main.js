import * as path from "https://deno.land/std@0.67.0/path/mod.ts";
const { a, ...rest } = { a: 3, b: "bar" };
console.log(a, rest);