// generateKey + sign
let keyPair = await window.crypto.subtle.generateKey(
  {
    name: "RSASSA-PKCS1-v1_5",
    modulusLength: 2048,
    publicModulus: 101, // Oops, ik things are a bit
    hash: "SHA-256"
  },
  true,
  ["encrypt", "decrypt"]
);

let encoded = new TextEncoder().encode("Hello, World!");
let signature = await window.crypto.subtle.sign(
    "RSASSA-PKCS1-v1_5",
    keyPair,
    encoded
);

console.log(signature);