// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

// A callback that throws, or whose isolate is terminated, must still give its
// native caller a return value instead of unwinding across the C boundary.
const definitions = {
  isize: { parameters: [], result: "isize" },
  usize: { parameters: [], result: "usize" },
  struct: { parameters: [], result: { struct: ["u32", "u64"] } },
} as const;

for (const [name, definition] of Object.entries(definitions)) {
  const callback = new Deno.UnsafeCallback(definition, () => {
    throw new Error("callback failed");
  });
  try {
    new Deno.UnsafeFnPointer(callback.pointer, definition).call();
    console.log(`${name}: returned without the error`);
  } catch (_err) {
    console.log(`${name}: thrown`);
  } finally {
    callback.close();
  }
}
