const key = await window.crypto.subtle.generateKey(
  {
    name: "RSA-PSS",
    modulusLength: 1024,
    publicExponent: new Uint8Array([1, 0, 1]),
    hash: "SHA-256",
  },
  true,
  ["sign", "verify"],
);
console.log(key);
const enc = new TextEncoder();
const encoded = enc.encode("Hey");
const signature = await window.crypto.subtle.sign(
  "RSA-PSS",
  key.privateKey,
  encoded,
);

console.log(signature);
