// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${targetDir}/${libPrefix}test_ffi.${libSuffix}`;

const lib = Deno.dlopen(libPath, {
  sleep_nonblocking: {
    name: "sleep_blocking",
    parameters: ["u64"],
    result: "void",
    nonblocking: true,
  },
});

const ONE_HOUR = 1000 * 60 * 60;
lib.symbols.sleep_nonblocking(ONE_HOUR).then(() => {
  console.log("WTF? We woke up!");
});

throw new Error("Error!");
