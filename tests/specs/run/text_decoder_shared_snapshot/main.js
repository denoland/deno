// Regression test: `TextDecoder.decode()` must snapshot a `SharedArrayBuffer`
// input into a private, non-shared buffer before decoding. Otherwise a worker
// mutating the shared bytes between V8's UTF-8 sizing pass and its write pass
// can make the decoder write more output than was allocated, corrupting native
// memory and crashing the process. This test races such a worker against many
// decodes and expects a clean exit rather than a segfault.

const iterations = 10000;
const byteLength = 1024 * 1024;
const sab = new SharedArrayBuffer(byteLength);
const words = new Uint16Array(sab);
words.fill(0xa9c2); // UTF-8 "©" (C2 A9): 2 input bytes -> 1 code unit.

const workerSource = `
  onmessage = ({ data }) => {
    const words = new Uint16Array(data);
    postMessage("ready");
    for (;;) {
      words.fill(0xa9c2);
      words.fill(0x4141); // ASCII "AA": same 2 bytes -> 2 code units.
    }
  };
`;
const workerUrl = URL.createObjectURL(
  new Blob([workerSource], { type: "text/javascript" }),
);
const worker = new Worker(workerUrl, { type: "module" });
const ready = new Promise((resolve) => worker.onmessage = resolve);
worker.postMessage(sab);
await ready;

const decoder = new TextDecoder("utf-8");
const input = new Uint8Array(sab);
let checksum = 0;
for (let i = 0; i < iterations; ++i) {
  checksum ^= decoder.decode(input).length;
}
worker.terminate();
// `checksum` is intentionally not asserted: the exact value depends on the
// race. Reaching this line without crashing is the assertion.
console.log("DONE");
Deno.exit(0);
