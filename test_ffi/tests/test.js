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
  "add": { parameters: ["u32", "u32"], result: "u32" },
});

dylib.symbols.print_something();
console.log(dylib.symbols.add(123, 456));

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
