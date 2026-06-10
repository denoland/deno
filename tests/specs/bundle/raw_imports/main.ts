import { message, value } from "./other.ts";
import otherText from "./other.ts" with { type: "text" };
import otherBytes from "./other.ts" with { type: "bytes" };

console.log("Normal import:", message, value);
console.log("Text import:");
console.log(otherText);
console.log("Bytes import:");
console.log(otherBytes);
