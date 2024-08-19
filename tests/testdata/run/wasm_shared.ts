const memory = new WebAssembly.Memory({
  initial: 1,
  maximum: 10,
  shared: true,
});
console.assert(memory.buffer instanceof SharedArrayBuffer);
