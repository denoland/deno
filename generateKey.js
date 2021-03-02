let key = await window.crypto.subtle.generateKey(
  {
    name: "RSA-OAEP",
    modulusLength: 4096,
    publicExponent: 2,
    hash: "SHA-256"
  },
  true,
  ["encrypt", "decrypt"]
);

console.log(key);