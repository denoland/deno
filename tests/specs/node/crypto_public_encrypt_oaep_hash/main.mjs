import crypto from "node:crypto";

const { publicKey, privateKey } = crypto.generateKeyPairSync("rsa", {
  modulusLength: 2048,
  publicKeyEncoding: { type: "spki", format: "pem" },
  privateKeyEncoding: { type: "pkcs8", format: "pem" },
});

// Invalid oaepHash should throw
try {
  crypto.publicEncrypt(
    {
      key: publicKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "this-is-an-invalid-hash-name",
    },
    Buffer.from("test"),
  );
  console.log("FAIL: did not throw for invalid oaepHash");
} catch {
  console.log("PASS: invalid oaepHash throws");
}

// sha256 roundtrip
{
  const encrypted = crypto.publicEncrypt(
    {
      key: publicKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "sha256",
    },
    Buffer.from("hello"),
  );
  const decrypted = crypto.privateDecrypt(
    {
      key: privateKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "sha256",
    },
    encrypted,
  );
  console.log("PASS: sha256 roundtrip:", decrypted.toString());
}

// sha384 roundtrip
{
  const encrypted = crypto.publicEncrypt(
    {
      key: publicKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "sha384",
    },
    Buffer.from("hello"),
  );
  const decrypted = crypto.privateDecrypt(
    {
      key: privateKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "sha384",
    },
    encrypted,
  );
  console.log("PASS: sha384 roundtrip:", decrypted.toString());
}

// sha512 roundtrip
{
  const encrypted = crypto.publicEncrypt(
    {
      key: publicKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "sha512",
    },
    Buffer.from("hello"),
  );
  const decrypted = crypto.privateDecrypt(
    {
      key: privateKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "sha512",
    },
    encrypted,
  );
  console.log("PASS: sha512 roundtrip:", decrypted.toString());
}

// Default sha1 still works
{
  const encrypted = crypto.publicEncrypt(
    {
      key: publicKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
    },
    Buffer.from("hello"),
  );
  const decrypted = crypto.privateDecrypt(
    {
      key: privateKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
    },
    encrypted,
  );
  console.log("PASS: default sha1 roundtrip:", decrypted.toString());
}

// Case-insensitive hash names
{
  const encrypted = crypto.publicEncrypt(
    {
      key: publicKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "SHA256",
    },
    Buffer.from("hello"),
  );
  const decrypted = crypto.privateDecrypt(
    {
      key: privateKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "SHA256",
    },
    encrypted,
  );
  console.log("PASS: SHA256 (uppercase) roundtrip:", decrypted.toString());
}

// WebCrypto-style hyphenated names
{
  const encrypted = crypto.publicEncrypt(
    {
      key: publicKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "SHA-256",
    },
    Buffer.from("hello"),
  );
  const decrypted = crypto.privateDecrypt(
    {
      key: privateKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "SHA-256",
    },
    encrypted,
  );
  console.log(
    "PASS: SHA-256 (WebCrypto-style) roundtrip:",
    decrypted.toString(),
  );
}

// sha3-256 roundtrip
{
  const encrypted = crypto.publicEncrypt(
    {
      key: publicKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "sha3-256",
    },
    Buffer.from("hello"),
  );
  const decrypted = crypto.privateDecrypt(
    {
      key: privateKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "sha3-256",
    },
    encrypted,
  );
  console.log("PASS: sha3-256 roundtrip:", decrypted.toString());
}

// md5 roundtrip
{
  const encrypted = crypto.publicEncrypt(
    {
      key: publicKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "md5",
    },
    Buffer.from("hello"),
  );
  const decrypted = crypto.privateDecrypt(
    {
      key: privateKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "md5",
    },
    encrypted,
  );
  console.log("PASS: md5 roundtrip:", decrypted.toString());
}

// Mismatched hash should fail
try {
  const encrypted = crypto.publicEncrypt(
    {
      key: publicKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "sha256",
    },
    Buffer.from("test"),
  );
  crypto.privateDecrypt(
    {
      key: privateKey,
      padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
      oaepHash: "sha1",
    },
    encrypted,
  );
  console.log("FAIL: mismatched hash should have thrown");
} catch {
  console.log("PASS: mismatched hash throws");
}
