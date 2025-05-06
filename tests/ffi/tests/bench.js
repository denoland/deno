// Copyright 2018-2025 the Deno authors. MIT license.
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
  "add_u32": { parameters: ["u32", "u32"], result: "u32" },
  "add_u64": { parameters: ["u64", "u64"], result: "u64" },
  "ffi_string": { parameters: [], result: "pointer" },
  "hash": { parameters: ["buffer", "u32"], result: "u32" },
  "nop_bool": { parameters: ["bool"], result: "void" },
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
  "nop_buffer": { parameters: ["buffer"], result: "void" },
  "return_bool": { parameters: [], result: "bool" },
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
  "return_buffer": { parameters: [], result: "buffer" },
  // Nonblocking calls
  "nop_nonblocking": {
    name: "nop",
    parameters: [],
    result: "void",
    nonblocking: true,
  },
  "nop_bool_nonblocking": {
    name: "nop_bool",
    parameters: ["bool"],
    result: "void",
    nonblocking: true,
  },
  "nop_u8_nonblocking": {
    name: "nop_u8",
    parameters: ["u8"],
    result: "void",
    nonblocking: true,
  },
  "nop_i8_nonblocking": {
    name: "nop_i8",
    parameters: ["i8"],
    result: "void",
    nonblocking: true,
  },
  "nop_u16_nonblocking": {
    name: "nop_u16",
    parameters: ["u16"],
    result: "void",
    nonblocking: true,
  },
  "nop_i16_nonblocking": {
    name: "nop_i16",
    parameters: ["i16"],
    result: "void",
    nonblocking: true,
  },
  "nop_u32_nonblocking": {
    name: "nop_u32",
    parameters: ["u32"],
    result: "void",
    nonblocking: true,
  },
  "nop_i32_nonblocking": {
    name: "nop_i32",
    parameters: ["i32"],
    result: "void",
    nonblocking: true,
  },
  "nop_u64_nonblocking": {
    name: "nop_u64",
    parameters: ["u64"],
    result: "void",
    nonblocking: true,
  },
  "nop_i64_nonblocking": {
    name: "nop_i64",
    parameters: ["i64"],
    result: "void",
    nonblocking: true,
  },
  "nop_usize_nonblocking": {
    name: "nop_usize",
    parameters: ["usize"],
    result: "void",
    nonblocking: true,
  },
  "nop_isize_nonblocking": {
    name: "nop_isize",
    parameters: ["isize"],
    result: "void",
    nonblocking: true,
  },
  "nop_f32_nonblocking": {
    name: "nop_f32",
    parameters: ["f32"],
    result: "void",
    nonblocking: true,
  },
  "nop_f64_nonblocking": {
    name: "nop_f64",
    parameters: ["f64"],
    result: "void",
    nonblocking: true,
  },
  "nop_buffer_nonblocking": {
    name: "nop_buffer",
    parameters: ["buffer"],
    result: "void",
    nonblocking: true,
  },
  "return_bool_nonblocking": {
    name: "return_bool",
    parameters: [],
    result: "bool",
    nonblocking: true,
  },
  "return_u8_nonblocking": {
    name: "return_u8",
    parameters: [],
    result: "u8",
    nonblocking: true,
  },
  "return_i8_nonblocking": {
    name: "return_i8",
    parameters: [],
    result: "i8",
    nonblocking: true,
  },
  "return_u16_nonblocking": {
    name: "return_u16",
    parameters: [],
    result: "u16",
    nonblocking: true,
  },
  "return_i16_nonblocking": {
    name: "return_i16",
    parameters: [],
    result: "i16",
    nonblocking: true,
  },
  "return_u32_nonblocking": {
    name: "return_u32",
    parameters: [],
    result: "u32",
    nonblocking: true,
  },
  "return_i32_nonblocking": {
    name: "return_i32",
    parameters: [],
    result: "i32",
    nonblocking: true,
  },
  "return_u64_nonblocking": {
    name: "return_u64",
    parameters: [],
    result: "u64",
    nonblocking: true,
  },
  "return_i64_nonblocking": {
    name: "return_i64",
    parameters: [],
    result: "i64",
    nonblocking: true,
  },
  "return_usize_nonblocking": {
    name: "return_usize",
    parameters: [],
    result: "usize",
    nonblocking: true,
  },
  "return_isize_nonblocking": {
    name: "return_isize",
    parameters: [],
    result: "isize",
    nonblocking: true,
  },
  "return_f32_nonblocking": {
    name: "return_f32",
    parameters: [],
    result: "f32",
    nonblocking: true,
  },
  "return_f64_nonblocking": {
    name: "return_f64",
    parameters: [],
    result: "f64",
    nonblocking: true,
  },
  "return_buffer_nonblocking": {
    name: "return_buffer",
    parameters: [],
    result: "buffer",
    nonblocking: true,
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
      "buffer",
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
      "buffer",
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
      "buffer",
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
      "buffer",
    ],
    result: "void",
    nonblocking: true,
  },
});

const { nop } = dylib.symbols;
Deno.bench("nop()", () => {
  nop();
});

const bytes = new Uint8Array(64);

const { hash } = dylib.symbols;
Deno.bench("hash()", () => {
  hash(bytes, bytes.byteLength);
});

const { ffi_string } = dylib.symbols;
Deno.bench(
  "c string",
  () => Deno.UnsafePointerView.getCString(ffi_string()),
);

const { add_u32 } = dylib.symbols;
Deno.bench("add_u32()", () => {
  add_u32(1, 2);
});

const { return_buffer } = dylib.symbols;
Deno.bench("return_buffer()", () => {
  return_buffer();
});

const { add_u64 } = dylib.symbols;
Deno.bench("add_u64() number", () => {
  add_u64(1, 2);
});

Deno.bench("add_u64() bigint", () => {
  add_u64(1n, 2n);
});

const { return_u64 } = dylib.symbols;
Deno.bench("return_u64()", () => {
  return_u64();
});

const { return_i64 } = dylib.symbols;
Deno.bench("return_i64()", () => {
  return_i64();
});

const { nop_bool } = dylib.symbols;
Deno.bench("nop_bool()", () => {
  nop_bool(true);
});

const { nop_u8 } = dylib.symbols;
Deno.bench("nop_u8()", () => {
  nop_u8(100);
});

const { nop_i8 } = dylib.symbols;
Deno.bench("nop_i8()", () => {
  nop_i8(100);
});

const { nop_u16 } = dylib.symbols;
Deno.bench("nop_u16()", () => {
  nop_u16(100);
});

const { nop_i16 } = dylib.symbols;
Deno.bench("nop_i16()", () => {
  nop_i16(100);
});

const { nop_u32 } = dylib.symbols;
Deno.bench("nop_u32()", () => {
  nop_u32(100);
});

const { nop_i32 } = dylib.symbols;
Deno.bench("nop_i32()", () => {
  nop_i32(100);
});

const { nop_u64 } = dylib.symbols;
Deno.bench("nop_u64() number", () => {
  nop_u64(100);
});

Deno.bench("nop_u64() bigint", () => {
  nop_u64(100n);
});

const { nop_i64 } = dylib.symbols;
Deno.bench("nop_i64() number", () => {
  nop_i64(100);
});

Deno.bench("nop_i64() bigint", () => {
  nop_i64(100n);
});

const { nop_usize } = dylib.symbols;
Deno.bench("nop_usize() number", () => {
  nop_usize(100);
});

Deno.bench("nop_usize() bigint", () => {
  nop_usize(100n);
});

const { nop_isize } = dylib.symbols;
Deno.bench("nop_isize() number", () => {
  nop_isize(100);
});

Deno.bench("nop_isize() bigint", () => {
  nop_isize(100n);
});

const { nop_f32 } = dylib.symbols;
Deno.bench("nop_f32()", () => {
  nop_f32(100.1);
});

const { nop_f64 } = dylib.symbols;
Deno.bench("nop_f64()", () => {
  nop_f64(100.1);
});

const { nop_buffer } = dylib.symbols;
const buffer = new Uint8Array(8).fill(5);
Deno.bench("nop_buffer()", () => {
  nop_buffer(buffer);
});

const { return_bool } = dylib.symbols;
Deno.bench("return_bool()", () => {
  return_bool();
});

const { return_u8 } = dylib.symbols;
Deno.bench("return_u8()", () => {
  return_u8();
});

const { return_i8 } = dylib.symbols;
Deno.bench("return_i8()", () => {
  return_i8();
});

const { return_u16 } = dylib.symbols;
Deno.bench("return_u16()", () => {
  return_u16();
});

const { return_i16 } = dylib.symbols;
Deno.bench("return_i16()", () => {
  return_i16();
});

const { return_u32 } = dylib.symbols;
Deno.bench("return_u32()", () => {
  return_u32();
});

const { return_i32 } = dylib.symbols;
Deno.bench("return_i32()", () => {
  return_i32();
});

const { return_usize } = dylib.symbols;
Deno.bench("return_usize()", () => {
  return_usize();
});

const { return_isize } = dylib.symbols;
Deno.bench("return_isize()", () => {
  return_isize();
});

const { return_f32 } = dylib.symbols;
Deno.bench("return_f32()", () => {
  return_f32();
});

const { return_f64 } = dylib.symbols;
Deno.bench("return_f64()", () => {
  return_f64();
});

// Nonblocking calls

const { nop_nonblocking } = dylib.symbols;
Deno.bench("nop_nonblocking()", async () => {
  await nop_nonblocking();
});

const { nop_bool_nonblocking } = dylib.symbols;
Deno.bench("nop_bool_nonblocking()", async () => {
  await nop_bool_nonblocking(true);
});

const { nop_u8_nonblocking } = dylib.symbols;
Deno.bench("nop_u8_nonblocking()", async () => {
  await nop_u8_nonblocking(100);
});

const { nop_i8_nonblocking } = dylib.symbols;
Deno.bench("nop_i8_nonblocking()", async () => {
  await nop_i8_nonblocking(100);
});

const { nop_u16_nonblocking } = dylib.symbols;
Deno.bench("nop_u16_nonblocking()", async () => {
  await nop_u16_nonblocking(100);
});

const { nop_i16_nonblocking } = dylib.symbols;
Deno.bench("nop_i16_nonblocking()", async () => {
  await nop_i16_nonblocking(100);
});

const { nop_u32_nonblocking } = dylib.symbols;
Deno.bench("nop_u32_nonblocking()", async () => {
  await nop_u32_nonblocking(100);
});

const { nop_i32_nonblocking } = dylib.symbols;

Deno.bench("nop_i32_nonblocking()", async () => {
  await nop_i32_nonblocking(100);
});

const { nop_u64_nonblocking } = dylib.symbols;
Deno.bench("nop_u64_nonblocking() number", async () => {
  await nop_u64_nonblocking(100);
});

Deno.bench("nop_u64_nonblocking() bigint", async () => {
  await nop_u64_nonblocking(100);
});

const { nop_i64_nonblocking } = dylib.symbols;
Deno.bench("nop_i64_nonblocking() number", async () => {
  await nop_i64_nonblocking(100);
});

Deno.bench("nop_i64_nonblocking() bigint", async () => {
  await nop_i64_nonblocking(100);
});

const { nop_usize_nonblocking } = dylib.symbols;
Deno.bench("nop_usize_nonblocking() number", async () => {
  await nop_usize_nonblocking(100);
});

Deno.bench("nop_usize_nonblocking() bigint", async () => {
  await nop_usize_nonblocking(100);
});

const { nop_isize_nonblocking } = dylib.symbols;
Deno.bench("nop_isize_nonblocking() number", async () => {
  await nop_isize_nonblocking(100);
});

Deno.bench("nop_isize_nonblocking() bigint", async () => {
  await nop_isize_nonblocking(100);
});

const { nop_f32_nonblocking } = dylib.symbols;
Deno.bench("nop_f32_nonblocking()", async () => {
  await nop_f32_nonblocking(100);
});

const { nop_f64_nonblocking } = dylib.symbols;
Deno.bench("nop_f64_nonblocking()", async () => {
  await nop_f64_nonblocking(100);
});

const { nop_buffer_nonblocking } = dylib.symbols;
Deno.bench("nop_buffer_nonblocking()", async () => {
  await nop_buffer_nonblocking(buffer);
});

const { return_bool_nonblocking } = dylib.symbols;
Deno.bench("return_bool_nonblocking()", async () => {
  await return_bool_nonblocking();
});

const { return_u8_nonblocking } = dylib.symbols;
Deno.bench("return_u8_nonblocking()", async () => {
  await return_u8_nonblocking();
});

const { return_i8_nonblocking } = dylib.symbols;
Deno.bench("return_i8_nonblocking()", async () => {
  await return_i8_nonblocking();
});

const { return_u16_nonblocking } = dylib.symbols;
Deno.bench("return_u16_nonblocking()", async () => {
  await return_u16_nonblocking();
});

const { return_i16_nonblocking } = dylib.symbols;
Deno.bench("return_i16_nonblocking()", async () => {
  await return_i16_nonblocking();
});

const { return_u32_nonblocking } = dylib.symbols;
Deno.bench("return_u32_nonblocking()", async () => {
  await return_u32_nonblocking();
});

const { return_i32_nonblocking } = dylib.symbols;
Deno.bench("return_i32_nonblocking()", async () => {
  await return_i32_nonblocking();
});

const { return_u64_nonblocking } = dylib.symbols;
Deno.bench("return_u64_nonblocking()", async () => {
  await return_u64_nonblocking();
});

const { return_i64_nonblocking } = dylib.symbols;
Deno.bench("return_i64_nonblocking()", async () => {
  await return_i64_nonblocking();
});

const { return_usize_nonblocking } = dylib.symbols;
Deno.bench("return_usize_nonblocking()", async () => {
  await return_usize_nonblocking();
});

const { return_isize_nonblocking } = dylib.symbols;
Deno.bench("return_isize_nonblocking()", async () => {
  await return_isize_nonblocking();
});

const { return_f32_nonblocking } = dylib.symbols;
Deno.bench("return_f32_nonblocking()", async () => {
  await return_f32_nonblocking();
});

const { return_f64_nonblocking } = dylib.symbols;
Deno.bench("return_f64_nonblocking()", async () => {
  await return_f64_nonblocking();
});

const { return_buffer_nonblocking } = dylib.symbols;
Deno.bench("return_buffer_nonblocking()", async () => {
  await return_buffer_nonblocking();
});

const { nop_many_parameters } = dylib.symbols;
const buffer2 = new Uint8Array(8).fill(25);
Deno.bench("nop_many_parameters()", () => {
  nop_many_parameters(
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

const { nop_many_parameters_nonblocking } = dylib.symbols;
Deno.bench("nop_many_parameters_nonblocking()", () => {
  nop_many_parameters_nonblocking(
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

Deno.bench("Deno.UnsafePointer.of", () => {
  Deno.UnsafePointer.of(buffer);
});

const bufferPointer = Deno.UnsafePointer.of(buffer);
Deno.bench("Deno.UnsafePointer.value", () => {
  Deno.UnsafePointer.value(bufferPointer);
});

const cstringBuffer = new TextEncoder().encode("Best believe it!\0");
const cstringPointerView = new Deno.UnsafePointerView(
  Deno.UnsafePointer.of(cstringBuffer),
);
Deno.bench("Deno.UnsafePointerView#getCString", () => {
  cstringPointerView.getCString();
});

const bufferPointerView = new Deno.UnsafePointerView(
  Deno.UnsafePointer.of(buffer),
);

Deno.bench("Deno.UnsafePointerView#getBool", () => {
  bufferPointerView.getBool();
});

Deno.bench("Deno.UnsafePointerView#getUint8", () => {
  bufferPointerView.getUint8();
});

Deno.bench("Deno.UnsafePointerView#getInt8", () => {
  bufferPointerView.getInt8();
});

Deno.bench("Deno.UnsafePointerView#getUint16", () => {
  bufferPointerView.getUint16();
});

Deno.bench("Deno.UnsafePointerView#getInt16", () => {
  bufferPointerView.getInt16();
});

Deno.bench("Deno.UnsafePointerView#getUint32", () => {
  bufferPointerView.getUint32();
});

Deno.bench("Deno.UnsafePointerView#getInt32", () => {
  bufferPointerView.getInt32();
});

Deno.bench("Deno.UnsafePointerView#getBigUint64", () => {
  bufferPointerView.getBigUint64();
});

Deno.bench("Deno.UnsafePointerView#getBigInt64", () => {
  bufferPointerView.getBigInt64();
});

Deno.bench("Deno.UnsafePointerView#getFloat32", () => {
  bufferPointerView.getFloat32();
});

Deno.bench("Deno.UnsafePointerView#getFloat64", () => {
  bufferPointerView.getFloat64();
});
