import { assert, assertEquals, unitTest } from "./test_util.ts";

unitTest(async function testGenerateRSAKey(): void {
    const subtle = window.crypto.subtle;
    assert(subtle);
    
    const keyPair = await subtle.generateKey(
        {
          name: "RSA-PSS",
          modulusLength: 2048,
          publicExponent: 65537,
          hash: "SHA-256"
        },
        true,
        ["sign", "verify"]
    );
    
    assert(keyPair.privateKey);
    assert(keyPair.publicKey);
    assertEquals(keyPair.privateKey.extractable, true);
    assert(keyPair.privateKey.usages.includes("sign"));
});