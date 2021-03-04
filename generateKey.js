const key = await window.crypto.subtle.generateKey(
  {
    name: "RSA-PSS",
    modulusLength: 1024,
    publicExponent: 65537,
    hash: "SHA-256"
  },
  true,
  ["sign", "verify"]
);
console.log(key)
const enc = new TextEncoder();
const encoded = enc.encode("Hey")
const signature = await window.crypto.subtle.sign(
  "RSA-PSS",
  key.privateKey,
  encoded
);

console.log(signature);