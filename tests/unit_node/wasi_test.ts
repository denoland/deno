// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-explicit-any camelcase
import { WASI } from "node:wasi";
import { assert, assertEquals, assertThrows } from "@std/assert";

// Compiled from:
//   (module
//     (import "wasi_snapshot_preview1" "fd_write"
//       (func $fd_write (param i32 i32 i32 i32) (result i32)))
//     (memory 1)
//     (export "memory" (memory 0))
//     (data (i32.const 8) "hello world\n")
//     (func $main (export "_start")
//       (i32.store (i32.const 0) (i32.const 8))
//       (i32.store (i32.const 4) (i32.const 12))
//       (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 20))
//       drop))
// deno-fmt-ignore
const HELLO_WASM = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x0c, 0x02, 0x60,
  0x04, 0x7f, 0x7f, 0x7f, 0x7f, 0x01, 0x7f, 0x60, 0x00, 0x00, 0x02, 0x23,
  0x01, 0x16, 0x77, 0x61, 0x73, 0x69, 0x5f, 0x73, 0x6e, 0x61, 0x70, 0x73,
  0x68, 0x6f, 0x74, 0x5f, 0x70, 0x72, 0x65, 0x76, 0x69, 0x65, 0x77, 0x31,
  0x08, 0x66, 0x64, 0x5f, 0x77, 0x72, 0x69, 0x74, 0x65, 0x00, 0x00, 0x03,
  0x02, 0x01, 0x01, 0x05, 0x03, 0x01, 0x00, 0x01, 0x07, 0x13, 0x02, 0x06,
  0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02, 0x00, 0x06, 0x5f, 0x73, 0x74,
  0x61, 0x72, 0x74, 0x00, 0x01, 0x0a, 0x1d, 0x01, 0x1b, 0x00, 0x41, 0x00,
  0x41, 0x08, 0x36, 0x02, 0x00, 0x41, 0x04, 0x41, 0x0c, 0x36, 0x02, 0x00,
  0x41, 0x01, 0x41, 0x00, 0x41, 0x01, 0x41, 0x14, 0x10, 0x00, 0x1a, 0x0b,
  0x0b, 0x12, 0x01, 0x00, 0x41, 0x08, 0x0b, 0x0c, 0x68, 0x65, 0x6c, 0x6c,
  0x6f, 0x20, 0x77, 0x6f, 0x72, 0x6c, 0x64, 0x0a,
]);

// (module (memory 1) (export "memory" (memory 0)))
// deno-fmt-ignore
const MEMONLY_WASM = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x05, 0x03, 0x01, 0x00,
  0x01, 0x07, 0x0a, 0x01, 0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02,
  0x00,
]);

Deno.test("[node/wasi] - WASI constructor validates version", () => {
  assertThrows(
    () => new WASI({ version: "invalid" as any }),
    TypeError,
  );
});

Deno.test("[node/wasi] - WASI constructor accepts preview1", () => {
  const wasi = new WASI({ version: "preview1" });
  assert(wasi);
  assert(wasi.wasiImport);
});

Deno.test("[node/wasi] - WASI constructor accepts unstable", () => {
  const wasi = new WASI({ version: "unstable" });
  assert(wasi);
});

Deno.test("[node/wasi] - getImportObject returns correct key for preview1", () => {
  const wasi = new WASI({ version: "preview1" });
  const importObj = wasi.getImportObject();
  assert("wasi_snapshot_preview1" in importObj);
});

Deno.test("[node/wasi] - getImportObject returns correct key for unstable", () => {
  const wasi = new WASI({ version: "unstable" });
  const importObj = wasi.getImportObject();
  assert("wasi_unstable" in importObj);
});

Deno.test("[node/wasi] - start() runs hello world wasm", () => {
  const wasi = new WASI({ version: "preview1" });
  const module = new WebAssembly.Module(HELLO_WASM);
  const instance = new WebAssembly.Instance(
    module,
    wasi.getImportObject() as WebAssembly.Imports,
  );
  const exitCode = wasi.start(instance);
  assertEquals(exitCode, 0);
});

Deno.test("[node/wasi] - start() throws if called twice", () => {
  const wasi = new WASI({ version: "preview1" });
  const module = new WebAssembly.Module(HELLO_WASM);
  const instance = new WebAssembly.Instance(
    module,
    wasi.getImportObject() as WebAssembly.Imports,
  );
  wasi.start(instance);
  assertThrows(() => wasi.start(instance), Error, "already started");
});

Deno.test("[node/wasi] - start() throws without _start export", () => {
  const wasi = new WASI({ version: "preview1" });
  const module = new WebAssembly.Module(MEMONLY_WASM);
  const instance = new WebAssembly.Instance(module, {});
  assertThrows(() => wasi.start(instance), Error, "_start");
});

Deno.test("[node/wasi] - WASI constructor with args and env", () => {
  const wasi = new WASI({
    version: "preview1",
    args: ["test", "--flag"],
    env: { FOO: "bar" },
  });
  assert(wasi);
});

Deno.test("[node/wasi] - WASI constructor coerces args, env, and preopens", () => {
  const wasi = new WASI({
    version: "preview1",
    args: [123, true] as any,
    env: { FOO: 1, BAR: false } as any,
    preopens: {
      "/sandbox": { toString: () => Deno.cwd() },
    } as any,
  });
  const module = new WebAssembly.Module(MEMONLY_WASM);
  const instance = new WebAssembly.Instance(module, {});
  wasi.initialize(instance);

  const memory = new Uint8Array(
    (instance.exports.memory as WebAssembly.Memory).buffer,
  );
  const view = new DataView(memory.buffer);

  assertEquals(wasi.wasiImport.args_sizes_get(0, 4), 0);
  assertEquals(view.getUint32(0, true), 2);
  assertEquals(view.getUint32(4, true), "123\0true\0".length);

  assertEquals(wasi.wasiImport.environ_sizes_get(8, 12), 0);
  assertEquals(view.getUint32(8, true), 2);
  assertEquals(view.getUint32(12, true), "FOO=1\0BAR=false\0".length);
});

Deno.test("[node/wasi] - WASI constructor with preopens", () => {
  const wasi = new WASI({
    version: "preview1",
    preopens: { "/sandbox": Deno.cwd() },
  });
  assert(wasi);
});

Deno.test("[node/wasi] - wasiImport has all expected functions", () => {
  const wasi = new WASI({ version: "preview1" });
  const expectedFunctions = [
    "args_get",
    "args_sizes_get",
    "environ_get",
    "environ_sizes_get",
    "clock_res_get",
    "clock_time_get",
    "random_get",
    "proc_exit",
    "fd_write",
    "fd_read",
    "fd_seek",
    "fd_close",
    "fd_fdstat_get",
    "fd_prestat_get",
    "fd_prestat_dir_name",
    "path_open",
    "sched_yield",
  ];

  for (const fn_name of expectedFunctions) {
    assertEquals(
      typeof wasi.wasiImport[fn_name],
      "function",
      `wasiImport.${fn_name} should be a function`,
    );
  }
});
