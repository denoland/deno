// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${targetDir}/${libPrefix}test_ffi.${libSuffix}`;

const dylib = Deno.dlopen(libPath, {
  "nop": { parameters: [], result: "void" },
  "nop_u8": { parameters: ["u8"], result: "void" },
  "nop_i8": { parameters: ["i8"], result: "void" },
  "nop_u16": { parameters: ["u16"], result: "void" },
  "nop_i16": { parameters: ["i16"], result: "void" },
  "nop_u32": { parameters: ["u32"], result: "void" },
  "nop_i32": { parameters: ["i32"], result: "void" },
  "nop_u64": { parameters: ["u64"], result: "void" },
  "nop_i64": { parameters: ["i64"], result: "void" },
  "nop_usize": { parameters: ["usize"], result: "void" },
  "nop_isize": { parameters: ["isize"], result: "void" },
  "nop_f32": { parameters: ["f32"], result: "void" },
  "nop_f64": { parameters: ["f64"], result: "void" },
  "nop_buffer": { parameters: ["pointer"], result: "void" },
  "return_u8": { parameters: [], result: "u8" },
  "return_i8": { parameters: [], result: "i8" },
  "return_u16": { parameters: [], result: "u16" },
  "return_i16": { parameters: [], result: "i16" },
  "return_u32": { parameters: [], result: "u32" },
  "return_i32": { parameters: [], result: "i32" },
  "return_u64": { parameters: [], result: "u64" },
  "return_i64": { parameters: [], result: "i64" },
  "return_usize": { parameters: [], result: "usize" },
  "return_isize": { parameters: [], result: "isize" },
  "return_f32": { parameters: [], result: "f32" },
  "return_f64": { parameters: [], result: "f64" },
  "return_buffer": { parameters: [], result: "pointer" },
  // Nonblocking calls
  "nop_nonblocking": { name: "nop", parameters: [], result: "void" },
  "nop_u8_nonblocking": { name: "nop_u8", parameters: ["u8"], result: "void" },
  "nop_i8_nonblocking": { name: "nop_i8", parameters: ["i8"], result: "void" },
  "nop_u16_nonblocking": {
    name: "nop_u16",
    parameters: ["u16"],
    result: "void",
  },
  "nop_i16_nonblocking": {
    name: "nop_i16",
    parameters: ["i16"],
    result: "void",
  },
  "nop_u32_nonblocking": {
    name: "nop_u32",
    parameters: ["u32"],
    result: "void",
  },
  "nop_i32_nonblocking": {
    name: "nop_i32",
    parameters: ["i32"],
    result: "void",
  },
  "nop_u64_nonblocking": {
    name: "nop_u64",
    parameters: ["u64"],
    result: "void",
  },
  "nop_i64_nonblocking": {
    name: "nop_i64",
    parameters: ["i64"],
    result: "void",
  },
  "nop_usize_nonblocking": {
    name: "nop_usize",
    parameters: ["usize"],
    result: "void",
  },
  "nop_isize_nonblocking": {
    name: "nop_isize",
    parameters: ["isize"],
    result: "void",
  },
  "nop_f32_nonblocking": {
    name: "nop_f32",
    parameters: ["f32"],
    result: "void",
  },
  "nop_f64_nonblocking": {
    name: "nop_f64",
    parameters: ["f64"],
    result: "void",
  },
  "nop_buffer_nonblocking": {
    name: "nop_buffer",
    parameters: ["pointer"],
    result: "void",
  },
  "return_u8_nonblocking": { name: "return_u8", parameters: [], result: "u8" },
  "return_i8_nonblocking": { name: "return_i8", parameters: [], result: "i8" },
  "return_u16_nonblocking": {
    name: "return_u16",
    parameters: [],
    result: "u16",
  },
  "return_i16_nonblocking": {
    name: "return_i16",
    parameters: [],
    result: "i16",
  },
  "return_u32_nonblocking": {
    name: "return_u32",
    parameters: [],
    result: "u32",
  },
  "return_i32_nonblocking": {
    name: "return_i32",
    parameters: [],
    result: "i32",
  },
  "return_u64_nonblocking": {
    name: "return_u64",
    parameters: [],
    result: "u64",
  },
  "return_i64_nonblocking": {
    name: "return_i64",
    parameters: [],
    result: "i64",
  },
  "return_usize_nonblocking": {
    name: "return_usize",
    parameters: [],
    result: "usize",
  },
  "return_isize_nonblocking": {
    name: "return_isize",
    parameters: [],
    result: "isize",
  },
  "return_f32_nonblocking": {
    name: "return_f32",
    parameters: [],
    result: "f32",
  },
  "return_f64_nonblocking": {
    name: "return_f64",
    parameters: [],
    result: "f64",
  },
  "return_buffer_nonblocking": {
    name: "return_buffer",
    parameters: [],
    result: "pointer",
  },
  // Parameter checking
  "nop_many_parameters": {
    parameters: [
      "u8",
      "i8",
      "u16",
      "i16",
      "u32",
      "i32",
      "u64",
      "i64",
      "usize",
      "isize",
      "f32",
      "f64",
      "pointer",
      "u8",
      "i8",
      "u16",
      "i16",
      "u32",
      "i32",
      "u64",
      "i64",
      "usize",
      "isize",
      "f32",
      "f64",
      "pointer",
    ],
    result: "void",
  },
  "nop_many_parameters_nonblocking": {
    name: "nop_many_parameters",
    parameters: [
      "u8",
      "i8",
      "u16",
      "i16",
      "u32",
      "i32",
      "u64",
      "i64",
      "usize",
      "isize",
      "f32",
      "f64",
      "pointer",
      "u8",
      "i8",
      "u16",
      "i16",
      "u32",
      "i32",
      "u64",
      "i64",
      "usize",
      "isize",
      "f32",
      "f64",
      "pointer",
    ],
    result: "void",
    nonblocking: true,
  },
});

Deno.bench("nop()", () => {
  dylib.symbols.nop();
});

Deno.bench("nop_u8()", () => {
  dylib.symbols.nop_u8(100);
});

Deno.bench("nop_i8()", () => {
  dylib.symbols.nop_i8(100);
});

Deno.bench("nop_u16()", () => {
  dylib.symbols.nop_u16(100);
});

Deno.bench("nop_i16()", () => {
  dylib.symbols.nop_i16(100);
});

Deno.bench("nop_u32()", () => {
  dylib.symbols.nop_u32(100);
});

Deno.bench("nop_i32()", () => {
  dylib.symbols.nop_i32(100);
});

Deno.bench("nop_u64()", () => {
  dylib.symbols.nop_u64(100);
});

Deno.bench("nop_i64()", () => {
  dylib.symbols.nop_i64(100);
});

Deno.bench("nop_usize()", () => {
  dylib.symbols.nop_usize(100);
});

Deno.bench("nop_isize()", () => {
  dylib.symbols.nop_isize(100);
});

Deno.bench("nop_f32()", () => {
  dylib.symbols.nop_f32(100);
});

Deno.bench("nop_f64()", () => {
  dylib.symbols.nop_f64(100);
});

const buffer = new Uint8Array(8).fill(5);
Deno.bench("nop_buffer()", () => {
  dylib.symbols.nop_buffer(buffer);
});

Deno.bench("return_u8()", () => {
  dylib.symbols.return_u8();
});

Deno.bench("return_i8()", () => {
  dylib.symbols.return_i8();
});

Deno.bench("return_u16()", () => {
  dylib.symbols.return_u16();
});

Deno.bench("return_i16()", () => {
  dylib.symbols.return_i16();
});

Deno.bench("return_u32()", () => {
  dylib.symbols.return_u32();
});

Deno.bench("return_i32()", () => {
  dylib.symbols.return_i32();
});

Deno.bench("return_u64()", () => {
  dylib.symbols.return_u64();
});

Deno.bench("return_i64()", () => {
  dylib.symbols.return_i64();
});

Deno.bench("return_usize()", () => {
  dylib.symbols.return_usize();
});

Deno.bench("return_isize()", () => {
  dylib.symbols.return_isize();
});

Deno.bench("return_f32()", () => {
  dylib.symbols.return_f32();
});

Deno.bench("return_f64()", () => {
  dylib.symbols.return_f64();
});

Deno.bench("return_buffer()", () => {
  dylib.symbols.return_buffer();
});

// Nonblocking calls

Deno.bench("nop_nonblocking()", async () => {
  await dylib.symbols.nop_nonblocking();
});

Deno.bench("nop_u8_nonblocking()", async () => {
  await dylib.symbols.nop_u8_nonblocking(100);
});

Deno.bench("nop_i8_nonblocking()", async () => {
  await dylib.symbols.nop_i8_nonblocking(100);
});

Deno.bench("nop_u16_nonblocking()", async () => {
  await dylib.symbols.nop_u16_nonblocking(100);
});

Deno.bench("nop_i16_nonblocking()", async () => {
  await dylib.symbols.nop_i16_nonblocking(100);
});

Deno.bench("nop_u32_nonblocking()", async () => {
  await dylib.symbols.nop_u32_nonblocking(100);
});

Deno.bench("nop_i32_nonblocking()", async () => {
  await dylib.symbols.nop_i32_nonblocking(100);
});

Deno.bench("nop_u64_nonblocking()", async () => {
  await dylib.symbols.nop_u64_nonblocking(100);
});

Deno.bench("nop_i64_nonblocking()", async () => {
  await dylib.symbols.nop_i64_nonblocking(100);
});

Deno.bench("nop_usize_nonblocking()", async () => {
  await dylib.symbols.nop_usize_nonblocking(100);
});

Deno.bench("nop_isize_nonblocking()", async () => {
  await dylib.symbols.nop_isize_nonblocking(100);
});

Deno.bench("nop_f32_nonblocking()", async () => {
  await dylib.symbols.nop_f32_nonblocking(100);
});

Deno.bench("nop_f64_nonblocking()", async () => {
  await dylib.symbols.nop_f64_nonblocking(100);
});

Deno.bench("nop_buffer_nonblocking()", async () => {
  await dylib.symbols.nop_buffer_nonblocking(buffer);
});

Deno.bench("return_u8_nonblocking()", async () => {
  await dylib.symbols.return_u8_nonblocking();
});

Deno.bench("return_i8_nonblocking()", async () => {
  await dylib.symbols.return_i8_nonblocking();
});

Deno.bench("return_u16_nonblocking()", async () => {
  await dylib.symbols.return_u16_nonblocking();
});

Deno.bench("return_i16_nonblocking()", async () => {
  await dylib.symbols.return_i16_nonblocking();
});

Deno.bench("return_u32_nonblocking()", async () => {
  await dylib.symbols.return_u32_nonblocking();
});

Deno.bench("return_i32_nonblocking()", async () => {
  await dylib.symbols.return_i32_nonblocking();
});

Deno.bench("return_u64_nonblocking()", async () => {
  await dylib.symbols.return_u64_nonblocking();
});

Deno.bench("return_i64_nonblocking()", async () => {
  await dylib.symbols.return_i64_nonblocking();
});

Deno.bench("return_usize_nonblocking()", async () => {
  await dylib.symbols.return_usize_nonblocking();
});

Deno.bench("return_isize_nonblocking()", async () => {
  await dylib.symbols.return_isize_nonblocking();
});

Deno.bench("return_f32_nonblocking()", async () => {
  await dylib.symbols.return_f32_nonblocking();
});

Deno.bench("return_f64_nonblocking()", async () => {
  await dylib.symbols.return_f64_nonblocking();
});

Deno.bench("return_buffer_nonblocking()", async () => {
  await dylib.symbols.return_buffer_nonblocking();
});

const buffer2 = new Uint8Array(8).fill(25);
Deno.bench("nop_many_parameters()", () => {
  dylib.symbols.nop_many_parameters(
    135,
    47,
    356,
    -236,
    7457,
    -1356,
    16471468n,
    -1334748136n,
    132658769535n,
    -42745856824n,
    13567.26437,
    7.686234e-3,
    buffer,
    64,
    -42,
    83,
    -136,
    3657,
    -2376,
    3277918n,
    -474628146n,
    344657895n,
    -2436732n,
    135.26437e3,
    264.3576468623546834,
    buffer2,
  );
});

Deno.bench("nop_many_parameters_nonblocking()", () => {
  dylib.symbols.nop_many_parameters_nonblocking(
    135,
    47,
    356,
    -236,
    7457,
    -1356,
    16471468n,
    -1334748136n,
    132658769535n,
    -42745856824n,
    13567.26437,
    7.686234e-3,
    buffer,
    64,
    -42,
    83,
    -136,
    3657,
    -2376,
    3277918n,
    -474628146n,
    344657895n,
    -2436732n,
    135.26437e3,
    264.3576468623546834,
    buffer2,
  );
});
