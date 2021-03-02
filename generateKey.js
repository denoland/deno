let key = await window.crypto.subtle.generateKey(
  {
    name: "RSA-OAEP",
    modulusLength: 1024,
    publicExponent: 65537,
    hash: "SHA-256"
  },
  true,
  ["encrypt", "decrypt"]
);

console.log(key);