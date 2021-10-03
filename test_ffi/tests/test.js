// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${targetDir}/${libPrefix}test_ffi.${libSuffix}`;

const resourcesPre = Deno.resources();
const dylib = Deno.dlopen(libPath, {
  "print_something": { parameters: [], result: "void" },
  "add_u32": { parameters: ["u32", "u32"], result: "u32" },
  "add_i32": { parameters: ["i32", "i32"], result: "i32" },
  "add_u64": { parameters: ["u64", "u64"], result: "u64" },
  "add_i64": { parameters: ["i64", "i64"], result: "i64" },
  "add_usize": { parameters: ["usize", "usize"], result: "usize" },
  "add_isize": { parameters: ["isize", "isize"], result: "isize" },
  "add_f32": { parameters: ["f32", "f32"], result: "f32" },
  "add_f64": { parameters: ["f64", "f64"], result: "f64" },
});

dylib.symbols.print_something();
console.log(dylib.symbols.add_u32(123, 456));
console.log(dylib.symbols.add_i32(123, 456));
console.log(dylib.symbols.add_u64(123, 456));
console.log(dylib.symbols.add_i64(123, 456));
console.log(dylib.symbols.add_usize(123, 456));
console.log(dylib.symbols.add_isize(123, 456));
console.log(dylib.symbols.add_f32(123.123, 456.789));
console.log(dylib.symbols.add_f64(123.123, 456.789));

dylib.close();
const resourcesPost = Deno.resources();

const preStr = JSON.stringify(resourcesPre, null, 2);
const postStr = JSON.stringify(resourcesPost, null, 2);
if (preStr !== postStr) {
  throw new Error(
    `Difference in open resources before dlopen and after closing:
Before: ${preStr}
After: ${postStr}`,
  );
}
console.log("Correct number of resources");
