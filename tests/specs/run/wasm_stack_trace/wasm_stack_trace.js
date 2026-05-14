import source unreachableModule from "./unreachable.wasm";
import source stackTraceModule from "./stack_trace.wasm";

// Test 1: Basic unreachable trap - verify wasm stack frame format
try {
  const instance = new WebAssembly.Instance(unreachableModule);
  instance.exports.main();
} catch (e) {
  console.log(e.stack);
}

// Test 2: Wasm -> JS -> throw - verify multi-frame wasm stack
try {
  const instance = new WebAssembly.Instance(stackTraceModule, {
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
const origPrepare = Error.prepareStackTrace;
Error.prepareStackTrace = (_error, callsites) => {
  return callsites
    .map((cs) => cs.toString())
    .join("\n");
};
try {
  const instance = new WebAssembly.Instance(unreachableModule);
  instance.exports.main();
} catch (e) {
  console.log(e.stack);
}
Error.prepareStackTrace = origPrepare;
