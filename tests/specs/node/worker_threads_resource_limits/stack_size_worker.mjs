import { parentPort } from "node:worker_threads";

// The test raises V8's JS stack limit to 3MB (--stack-size=3072), which is more
// than Rust's default 2MB OS thread stack. If the worker's thread doesn't
// actually get the stack size we report in `resourceLimits.stackSizeMb`, this
// recursion runs off the end of the OS stack and aborts the whole process
// instead of raising RangeError.
let depth = 0;
function recurse() {
  depth++;
  recurse();
}

try {
  recurse();
} catch (err) {
  parentPort.postMessage({ caught: err instanceof RangeError });
}
