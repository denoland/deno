// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const filenameBase = "test_ffi";

let filenameSuffix = ".so";
let filenamePrefix = "lib";

if (Deno.build.os === "windows") {
  filenameSuffix = ".dll";
  filenamePrefix = "";
}
if (Deno.build.os === "darwin") {
  filenameSuffix = ".dylib";
}

const filename = `../target/${Deno.args[0]}/${filenamePrefix}${filenameBase}${filenameSuffix}`;

console.log("Deno.resources():", Deno.resources());

const lib = Deno.loadLibrary(filename);

console.log("Deno.resources():", Deno.resources());

lib.call("print_something");

console.log(
  "add_one_i8",
  lib.call("add_one_i8", {
    params: [{ typeName: "i8", value: 1 }],
    returnType: "i8",
  })
);

console.log(
  "add_one_i16",
  lib.call("add_one_i16", {
    params: [{ typeName: "i16", value: 1 }],
    returnType: "i16",
  })
);

console.log(
  "add_one_i32",
  lib.call("add_one_i32", {
    params: [{ typeName: "i32", value: 1 }],
    returnType: "i32",
  })
);

console.log(
  "add_one_i64",
  lib.call("add_one_i64", {
    params: [{ typeName: "i64", value: 1 }],
    returnType: "i64",
  })
);

console.log(
  "add_one_u8",
  lib.call("add_one_u8", {
    params: [{ typeName: "u8", value: 1 }],
    returnType: "u8",
  })
);

console.log(
  "add_one_u16",
  lib.call("add_one_u16", {
    params: [{ typeName: "u16", value: 1 }],
    returnType: "u16",
  })
);

console.log(
  "add_one_u32",
  lib.call("add_one_u32", {
    params: [{ typeName: "u32", value: 1 }],
    returnType: "u32",
  })
);

console.log(
  "add_one_u64",
  lib.call("add_one_i64", {
    params: [{ typeName: "i64", value: 1 }],
    returnType: "i64",
  })
);

console.log(
  "add_one_f32",
  lib.call("add_one_f32", {
    params: [{ typeName: "f32", value: 2.5 }],
    returnType: "f32",
  })
);

console.log(
  "add_one_f64",
  lib.call("add_one_f64", {
    params: [{ typeName: "f64", value: 2.14 }],
    returnType: "f64",
  })
);

lib.close();

console.log("Deno.resources():", Deno.resources());
