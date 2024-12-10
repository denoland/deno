import { generateKeyPairSync } from "node:crypto";

const modulusLength = 4096;

const key = generateKeyPairSync("rsa", {
  modulusLength,
  publicKeyEncoding: {
    format: "jwk",
  },
  privateKeyEncoding: {
    format: "jwk",
  },
});

console.log(key);
