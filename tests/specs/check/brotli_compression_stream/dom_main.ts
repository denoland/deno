// Same regression as main.ts but type-checked against the `dom` lib (see
// tsconfig.json in this directory).
const compression: CompressionStream = new CompressionStream("brotli");
const decompression: DecompressionStream = new DecompressionStream("brotli");
console.log(compression, decompression);
