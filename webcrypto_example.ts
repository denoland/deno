const key = await window.crypto.subtle.generateKey(
  {
    name: "ECDSA",
    namedCurve: "P-384",
  },
  true,
  ["sign", "verify"],
);

const enc = new TextEncoder();
const encoded = enc.encode("Hey");
const signature = await window.crypto.subtle.sign(
  { name: "ECDSA", hash: "SHA-384" },
  key.privateKey,
  encoded,
);

console.log(signature);
