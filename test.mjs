import { deflateSync } from "node:zlib";
import { Buffer } from "node:buffer";
const data = deflateSync(Buffer.from("Hello World"));
console.log(new TextDecoder().decode(data));
