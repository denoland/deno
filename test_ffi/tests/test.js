// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file

const filenameBase = "test_ffi";

const filenameSuffix = Deno.build.os === "darwin"
  ? ".dylib"
  : Deno.build.os === "windows"
  ? ".dll"
  : ".so";
const filenamePrefix = Deno.build.os === "windows" ? "" : "lib";

const filename = `../target/${
  Deno.args[0]
}/${filenamePrefix}${filenameBase}${filenameSuffix}`;

const resourcesPre = Deno.resources();
const dylibRid = Deno.dlopen(filename);

console.log(`Dylib rid: ${dylibRid}`);

Deno.dlcall(dylibRid, {
  sym: "print_something",
  args: [],
  returnType: "void",
});

console.log(`${
  Deno.dlcall(dylibRid, {
    sym: "add_two",
    args: [{ argType: "u32", value: 123 }],
    returnType: "u32",
  })
}`);

Deno.close(dylibRid);
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
