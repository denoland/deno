import { Buffer } from "node:buffer";

// Normal indexOf should work
const buf = Buffer.from("Hello World");
console.log(buf.indexOf("World"));
console.log(buf.indexOf("World", 0, "utf8"));
console.log(buf.includes("Hello"));

// indexOf with Buffer argument
const target = Buffer.from("World");
console.log(buf.indexOf(target));

console.log("ok");
