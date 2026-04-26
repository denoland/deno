import crypto from "node:crypto";

const { privateKey, publicKey } = crypto.generateKeyPairSync("rsa", {
  modulusLength: 2048,
});

const testData = new Uint8Array([1, 2, 3, 4]);

// Test 1: createSign with RSA PSS padding and various salt lengths
function sign(
  key: crypto.KeyObject,
  data: Uint8Array,
  saltLength: number,
): Buffer {
  return crypto
    .createSign("sha256")
    .update(data)
    .sign({
      key: key,
      padding: crypto.constants.RSA_PKCS1_PSS_PADDING,
      saltLength,
    });
}

const sig20 = sign(privateKey, testData, 20);
const sig32 = sign(privateKey, testData, 32);
const sig64 = sign(privateKey, testData, 64);

console.log("Salt length 20:", sig20.length);
console.log("Salt length 32:", sig32.length);
console.log("Salt length 64:", sig64.length);

// Test 2: Signatures with different salt lengths should be different
// (PSS is probabilistic, so even same salt length gives different sigs,
// but we just verify they're all 256 bytes for a 2048-bit key)
console.log(
  "All signatures are 256 bytes:",
  [sig20, sig32, sig64].every((s) => s.length === 256),
);

// Test 3: Verify signatures with createVerify
function verify(
  key: crypto.KeyObject,
  data: Uint8Array,
  signature: Buffer,
  saltLength: number,
): boolean {
  return crypto
    .createVerify("sha256")
    .update(data)
    .verify(
      {
        key: key,
        padding: crypto.constants.RSA_PKCS1_PSS_PADDING,
        saltLength,
      },
      signature,
    );
}

console.log("Verify salt 20:", verify(publicKey, testData, sig20, 20));
console.log("Verify salt 32:", verify(publicKey, testData, sig32, 32));
console.log("Verify salt 64:", verify(publicKey, testData, sig64, 64));

// Test 4: Cross-verify should fail (different salt lengths)
console.log(
  "Cross-verify (sig20, salt32):",
  verify(publicKey, testData, sig20, 32),
);

// Test 5: crypto.sign / crypto.verify one-shot with PSS padding
const oneShotSig = crypto.sign("sha256", testData, {
  key: privateKey,
  padding: crypto.constants.RSA_PKCS1_PSS_PADDING,
  saltLength: 32,
});
console.log("One-shot sign length:", oneShotSig.length);

const oneShotVerify = crypto.verify(
  "sha256",
  testData,
  {
    key: publicKey,
    padding: crypto.constants.RSA_PKCS1_PSS_PADDING,
    saltLength: 32,
  },
  oneShotSig,
);
console.log("One-shot verify:", oneShotVerify);

console.log("ok");
