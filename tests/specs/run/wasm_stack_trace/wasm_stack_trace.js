// Test 1: Basic unreachable trap - verify wasm stack frame format
console.log("=== Test 1: unreachable trap ===");
try {
  const binary = Deno.readFileSync("./unreachable.wasm");
  const module = new WebAssembly.Module(binary);
  const instance = new WebAssembly.Instance(module);
  instance.exports.main();
} catch (e) {
  console.log(e.stack);
}

// Test 2: Wasm -> JS -> throw - verify multi-frame wasm stack
console.log("\n=== Test 2: wasm call chain ===");
try {
  const binary = Deno.readFileSync("./stack_trace.wasm");
  const module = new WebAssembly.Module(binary);
  const instance = new WebAssembly.Instance(module, {
    env: {
      js_func() {
        throw new Error("from JS");
      },
    },
  });
  instance.exports.entry();
} catch (e) {
  console.log(e.stack);
}

// Test 3: Error.prepareStackTrace sees correct wasm frame toString
console.log("\n=== Test 3: prepareStackTrace ===");
const origPrepare = Error.prepareStackTrace;
Error.prepareStackTrace = (_error, callsites) => {
  return callsites
    .map((cs) => cs.toString())
    .join("\n");
};
try {
  const binary = Deno.readFileSync("./unreachable.wasm");
  const module = new WebAssembly.Module(binary);
  const instance = new WebAssembly.Instance(module);
  instance.exports.main();
} catch (e) {
  console.log(e.stack);
}
Error.prepareStackTrace = origPrepare;
