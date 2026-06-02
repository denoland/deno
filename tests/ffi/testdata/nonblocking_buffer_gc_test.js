// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file

// Test that nonblocking FFI calls retain ArrayBuffer backing stores
// so they are not freed by GC while the native call is in progress.
// Run with: --v8-flags=--expose-gc

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${targetDir}/${libPrefix}test_ffi.${libSuffix}`;

const dylib = Deno.dlopen(libPath, {
  nonblocking_buffer_checksum: {
    parameters: ["buffer", "usize", "u8"],
    result: "u64",
    nonblocking: true,
  },
});

const BUF_SIZE = 256;
const FILL_VALUE = 0x42;

// Launch the nonblocking FFI call from a function scope so the buffer
// becomes unreachable after the call is dispatched.
function launchNonblockingCall() {
  const buf = new Uint8Array(BUF_SIZE);
  buf.fill(FILL_VALUE);
  return dylib.symbols.nonblocking_buffer_checksum(
    buf,
    BUF_SIZE,
    FILL_VALUE,
  );
}

const promise = launchNonblockingCall();

// Force garbage collection while the native function is sleeping.
// Without the fix, this would free the ArrayBuffer backing store,
// causing the native function to read freed memory.
gc();

const checksum = await promise;
const expected = BigInt(FILL_VALUE * BUF_SIZE);

if (checksum !== expected) {
  console.log(`FAIL: checksum ${checksum} !== expected ${expected}`);
  Deno.exit(1);
}

console.log("nonblocking_buffer_gc_test passed");
dylib.close();
