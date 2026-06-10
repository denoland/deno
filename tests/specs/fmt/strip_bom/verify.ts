import bytes from "./main.ts" with { type: "bytes" };

// should not have bom
console.log(bytes.length);
