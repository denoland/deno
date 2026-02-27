// Copyright 2018-2025 the Deno authors. MIT license.

// A minimal wasm module with a single exported function that triggers `unreachable`.
// WAT equivalent:
//   (module
//     (func (export "trigger_unreachable")
//       unreachable
//     )
//   )
const wasmBytes = new Uint8Array([
  0x00,
  0x61,
  0x73,
  0x6d, // magic
  0x01,
  0x00,
  0x00,
  0x00, // version
  // Type section: one function type () -> ()
  0x01,
  0x04,
  0x01,
  0x60,
  0x00,
  0x00,
  // Function section: one function, type index 0
  0x03,
  0x02,
  0x01,
  0x00,
  // Export section: export "trigger_unreachable" as function index 0
  0x07,
  0x17,
  0x01,
  0x13,
  0x74,
  0x72,
  0x69,
  0x67,
  0x67,
  0x65,
  0x72,
  0x5f,
  0x75,
  0x6e,
  0x72,
  0x65,
  0x61,
  0x63,
  0x68,
  0x61,
  0x62,
  0x6c,
  0x65,
  0x00,
  0x00,
  // Code section: one function body with `unreachable` instruction
  0x0a,
  0x05,
  0x01,
  0x03,
  0x00,
  0x00,
  0x0b,
]);

const wasmModule = new WebAssembly.Module(wasmBytes);
const instance = new WebAssembly.Instance(wasmModule);
const trigger = instance.exports.trigger_unreachable as () => void;

// Test 1: Default stack trace formatting
try {
  trigger();
} catch (e) {
  const stack = (e as Error).stack!;
  // Verify wasm frames use the W3C spec format: wasm-function[N]:0x<hex>
  const lines = stack.split("\n");
  console.log("=== Default stack trace ===");
  for (const line of lines) {
    if (line.includes("wasm-function")) {
      // Normalize the wasm URL hash since it may vary
      console.log(
        line.replace(/wasm:\/\/wasm\/[a-f0-9]+/, "wasm://wasm/<hash>"),
      );
    }
  }
}

// Test 2: User prepareStackTrace sees correct wasm callsite toString()
// deno-lint-ignore no-explicit-any
(Error as any).prepareStackTrace = function (_: unknown, callsites: any[]) {
  const parts: string[] = [];
  for (const cs of callsites) {
    const fileName = cs.getFileName();
    if (fileName && fileName.startsWith("wasm://")) {
      const str = cs.toString();
      // Normalize the wasm URL hash
      parts.push(str.replace(/wasm:\/\/wasm\/[a-f0-9]+/, "wasm://wasm/<hash>"));
    }
  }
  return parts.join("\n");
};
try {
  trigger();
} catch (e) {
  console.log("=== prepareStackTrace callsite.toString() ===");
  console.log((e as Error).stack);
}
