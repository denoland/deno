// https://github.com/denoland/deno/issues/12263
// Test for a panic that happens when a worker is closed in the reactions of a
// WASM async operation.

// The minimum valid wasm module, plus two additional zero bytes.
const buffer = new Uint8Array([
  0x00,
  0x61,
  0x73,
  0x6D,
  0x01,
  0x00,
  0x00,
  0x00,
  0x00,
  0x00,
]);
WebAssembly.compile(buffer).catch((err) => {
  console.log("Error:", err);
  self.close();
});
