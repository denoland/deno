import { brotliCompressSync } from "node:zlib";
import { Buffer } from "node:buffer";

console.log(brotliCompressSync(Buffer.from("hello")));
