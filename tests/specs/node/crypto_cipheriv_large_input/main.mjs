import crypto from "node:crypto";

// Test that cipheriv.update throws on input >= 2**31 - 1 bytes,
// matching Node.js/OpenSSL behavior.
try {
  crypto
    .createCipheriv("aes-128-gcm", Buffer.alloc(16), Buffer.alloc(12))
    .update(Buffer.allocUnsafeSlow(2 ** 31 - 1));
  console.log("ERROR: should have thrown");
} catch (error) {
  console.log("Caught:", error.message);
}

console.log("Process still alive after try/catch");
