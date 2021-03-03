let key = await window.crypto.subtle.generateKey(
  {
    name: "RSA-PSS",
    modulusLength: 1024,
    publicExponent: 65537,
    hash: "SHA-256"
  },
  true,
  ["encrypt", "decrypt"]
);
console.log(key)
let enc = new TextEncoder();
let encoded = enc.encode("Hey")
let signature = await window.crypto.subtle.sign(
  "RSA-PSS",
  key.privateKey,
  encoded
);

console.log(signature);