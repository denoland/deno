// Regression test: AbortSignal.any() dependent signals must not be GC'd while
// they have abort listeners and a source timer keeps the event loop alive.
//
// The dependent signal from AbortSignal.any() is only held via WeakRef in the
// source signal's dependentSignals set. Without a strong reference, GC collects
// it, the abort listener is lost, and the promise never resolves.

declare const gc: (opts?: object) => void;

const ac = new AbortController();
const { promise, resolve } = Promise.withResolvers<void>();

// Wrap signal creation in a function so the dependent signal is not held
// on the current stack frame when GC runs.
function setup() {
  AbortSignal.any([
    AbortSignal.timeout(500),
    ac.signal,
  ]).addEventListener("abort", () => resolve());
}
setup();

// GC must run from a fresh macrotask so the setup() stack frame is fully
// gone and V8 can see the dependent signal as unreachable (only weak refs).
setTimeout(() => {
  gc({ type: "major", execution: "sync" });
}, 10);

// Without the fix the dependent signal is collected, the abort listener
// disappears, and this await hangs until the event loop exits with:
// "Promise resolution is still pending but the event loop has already resolved"
await promise;
console.log("ok");
