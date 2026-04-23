import { transcode } from "node:buffer";

// Odd-length buffer: trailing byte should be ignored
// (matches Node.js behavior)
const odd = Buffer.from([0x61, 0x00, 0x62]);
console.log(transcode(odd, "utf16le", "utf8").toString());

// Even-length buffer: normal case
const even = Buffer.from([0x48, 0x00, 0x69, 0x00]);
console.log(transcode(even, "utf16le", "utf8").toString());

// Empty buffer
const empty = Buffer.alloc(0);
console.log(transcode(empty, "utf16le", "utf8").toString());

// Single byte (all trailing)
const single = Buffer.from([0x41]);
console.log(transcode(single, "utf16le", "utf8").toString());

console.log("ok");
