import {
  createPrivateKey,
  createPublicKey,
  createSecretKey,
  webcrypto,
} from "node:crypto";

// 1. RSA: createPublicKey accepts a public CryptoKey, then derives a public
//    KeyObject from a private CryptoKey.
const rsa = await webcrypto.subtle.generateKey(
  {
    name: "RSASSA-PKCS1-v1_5",
    modulusLength: 2048,
    publicExponent: new Uint8Array([1, 0, 1]),
    hash: "SHA-256",
  },
  true,
  ["sign", "verify"],
);

const rsaPubFromPub = createPublicKey(rsa.publicKey);
console.log(
  "rsa pub-from-pub:",
  rsaPubFromPub.type,
  rsaPubFromPub.asymmetricKeyType,
);

const rsaPubFromPriv = createPublicKey(rsa.privateKey);
console.log(
  "rsa pub-from-priv:",
  rsaPubFromPriv.type,
  rsaPubFromPriv.asymmetricKeyType,
);

// 2. createPrivateKey accepts a CryptoKey both directly and via { key }.
const rsaPriv = createPrivateKey(rsa.privateKey);
console.log("rsa priv direct:", rsaPriv.type, rsaPriv.asymmetricKeyType);

const rsaPriv2 = createPrivateKey({ key: rsa.privateKey });
console.log("rsa priv via key:", rsaPriv2.type, rsaPriv2.asymmetricKeyType);

// 3. Round-trip: CryptoKey -> KeyObject -> PEM export.
const pem = rsaPubFromPub.export({ type: "spki", format: "pem" }) as string;
console.log("pem starts:", pem.startsWith("-----BEGIN PUBLIC KEY-----"));
console.log("pem ends:", pem.trimEnd().endsWith("-----END PUBLIC KEY-----"));

// 4. createSecretKey accepts an HMAC CryptoKey.
const secret = await webcrypto.subtle.generateKey(
  { name: "HMAC", hash: "SHA-256", length: 256 },
  true,
  ["sign", "verify"],
);
const secretKo = createSecretKey(secret);
console.log("secret:", secretKo.type, secretKo.symmetricKeySize);

// 5. createSecretKey rejects an asymmetric CryptoKey.
let rejected = false;
try {
  createSecretKey(rsa.publicKey);
} catch (e) {
  rejected = e instanceof Error && /secret/.test(e.message);
}
console.log("rejects asymmetric:", rejected);

// 6. Passing a public CryptoKey to createPrivateKey is a type error.
let privRejected = false;
try {
  createPrivateKey(rsa.publicKey);
} catch (e) {
  privRejected = e instanceof Error;
}
console.log("rejects public-as-private:", privRejected);

console.log("ok");
