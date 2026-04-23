import { Buffer } from "node:buffer";

const a = Buffer.from("abc");
const b = Buffer.from("abc");

// Valid compare should work
console.log(a.compare(b, 0, 3, 0, 3));

// Invalid sourceStart should throw ERR_OUT_OF_RANGE
try {
  a.compare(b, 0, 3, -1, 3);
  console.log("FAIL: should have thrown for negative sourceStart");
} catch (e: any) {
  console.log(`sourceStart: ${e.code}`);
}

// Invalid sourceEnd (exceeds buffer length) should throw
try {
  a.compare(b, 0, 3, 0, 100);
  console.log("FAIL: should have thrown for sourceEnd > length");
} catch (e: any) {
  console.log(`sourceEnd: ${e.code}`);
}

console.log("done");
