const key = await window.crypto.subtle.generateKey(
  {
    name: "HMAC",
    hash: "SHA-512"
  },
  true,
  ["sign", "verify"]
);

const enc = new TextEncoder();
const encoded = enc.encode("Hey");

let signature = await window.crypto.subtle.sign(
  "HMAC",
  key,
  encoded
);

console.log(signature);
