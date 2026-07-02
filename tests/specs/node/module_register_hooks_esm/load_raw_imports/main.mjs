import bytes from "./data.bin" with { type: "bytes" };
import text from "./data.txt" with { type: "text" };

console.log("bytes:", bytes.byteLength);
console.log("text:", JSON.stringify(text));
