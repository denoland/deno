// Copyright 2018-2026 the Deno authors. MIT license.

import {
  assert,
  assertEquals,
  assertNotEquals,
  assertRejects,
  assertThrows,
} from "./test_util.ts";

// deno-lint-ignore no-explicit-any
type AnyAlg = any;
// deno-lint-ignore no-explicit-any
type AnyKey = any;

// Cast over the static `KeyFormat` overloads so we can pass the extended
// ML-DSA formats (`raw-seed`, `raw-public`, `raw-private`).
const exportKey = (
  format: string,
  key: CryptoKey,
): Promise<ArrayBuffer> =>
  // deno-lint-ignore no-explicit-any
  (crypto.subtle.exportKey as any)(format, key);

const importKey = (
  format: string,
  data: BufferSource,
  algorithm: AnyAlg,
  extractable: boolean,
  usages: KeyUsage[],
): Promise<CryptoKey> =>
  // deno-lint-ignore no-explicit-any
  (crypto.subtle.importKey as any)(
    format,
    data,
    algorithm,
    extractable,
    usages,
  );

const variants = [
  ["ML-DSA-44", 1312, 2560, 2420],
  ["ML-DSA-65", 1952, 4032, 3309],
  ["ML-DSA-87", 2592, 4896, 4627],
] as const;

for (const [name, pubLen, privLen, sigLen] of variants) {
  Deno.test(`webcrypto ${name} generate/sign/verify`, async () => {
    const { publicKey, privateKey } = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;

    assertEquals(privateKey.type, "private");
    assertEquals(publicKey.type, "public");
    assertEquals(privateKey.algorithm.name, name);
    assertEquals(publicKey.algorithm.name, name);
    assertEquals(privateKey.usages, ["sign"]);
    assertEquals(publicKey.usages, ["verify"]);

    const data = new TextEncoder().encode("hello world");
    const sig = await crypto.subtle.sign({ name } as AnyAlg, privateKey, data);
    assertEquals(sig.byteLength, sigLen);

    const ok = await crypto.subtle.verify(
      { name } as AnyAlg,
      publicKey,
      sig,
      data,
    );
    assert(ok);

    // Bad signature.
    const bad = new Uint8Array(sig.byteLength);
    const badOk = await crypto.subtle.verify(
      { name } as AnyAlg,
      publicKey,
      bad,
      data,
    );
    assert(!badOk);

    // Mutated message should fail verification.
    const mutated = new Uint8Array(data);
    mutated[0] ^= 1;
    const ok2 = await crypto.subtle.verify(
      { name } as AnyAlg,
      publicKey,
      sig,
      mutated,
    );
    assert(!ok2);
  });

  Deno.test(`webcrypto ${name} sign with empty context`, async () => {
    const { publicKey, privateKey } = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;

    const data = new TextEncoder().encode("ctx payload");
    const sig = await crypto.subtle.sign(
      { name, context: new Uint8Array(0) } as AnyAlg,
      privateKey,
      data,
    );
    const ok = await crypto.subtle.verify(
      { name, context: new Uint8Array(0) } as AnyAlg,
      publicKey,
      sig,
      data,
    );
    assert(ok);
  });

  Deno.test(`webcrypto ${name} non-empty context unsupported`, async () => {
    const { privateKey } = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;
    const data = new TextEncoder().encode("payload");
    await assertRejects(
      async () => {
        await crypto.subtle.sign(
          { name, context: new Uint8Array([1, 2, 3]) } as AnyAlg,
          privateKey,
          data,
        );
      },
      DOMException,
    );
  });

  Deno.test(`webcrypto ${name} export raw-public / raw-private / raw-seed`, async () => {
    const { publicKey, privateKey } = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;

    const rawPub = new Uint8Array(await exportKey("raw-public", publicKey));
    assertEquals(rawPub.byteLength, pubLen);

    const rawPriv = new Uint8Array(await exportKey("raw-private", privateKey));
    assertEquals(rawPriv.byteLength, privLen);

    const rawSeed = new Uint8Array(await exportKey("raw-seed", privateKey));
    assertEquals(rawSeed.byteLength, 32);
  });

  Deno.test(`webcrypto ${name} import raw-seed round-trip`, async () => {
    const seed = new Uint8Array(32);
    crypto.getRandomValues(seed);

    const priv1 = await importKey(
      "raw-seed",
      seed,
      { name },
      true,
      ["sign"],
    );
    const priv2 = await importKey(
      "raw-seed",
      seed,
      { name },
      true,
      ["sign"],
    );

    // Both must produce the same expanded raw-private bytes.
    const raw1 = new Uint8Array(await exportKey("raw-private", priv1));
    const raw2 = new Uint8Array(await exportKey("raw-private", priv2));
    assertEquals(raw1, raw2);

    // Seed export equals the seed we imported.
    const seedOut = new Uint8Array(await exportKey("raw-seed", priv1));
    assertEquals(seedOut, seed);
  });

  Deno.test(`webcrypto ${name} import raw-private round-trip + sign/verify`, async () => {
    const original = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;
    const rawPriv = new Uint8Array(
      await exportKey("raw-private", original.privateKey),
    );
    const rawPub = new Uint8Array(
      await exportKey("raw-public", original.publicKey),
    );

    const priv = await importKey(
      "raw-private",
      rawPriv,
      { name },
      true,
      ["sign"],
    );
    const pub = await importKey(
      "raw-public",
      rawPub,
      { name },
      true,
      ["verify"],
    );

    const msg = new TextEncoder().encode("via raw");
    const sig = await crypto.subtle.sign({ name } as AnyAlg, priv, msg);
    const ok = await crypto.subtle.verify({ name } as AnyAlg, pub, sig, msg);
    assert(ok);

    // raw-seed should not be available for keys imported from raw-private.
    await assertRejects(
      async () => {
        await exportKey("raw-seed", priv);
      },
      DOMException,
    );
  });

  Deno.test(`webcrypto ${name} pkcs8/spki round-trip + sign/verify`, async () => {
    const orig = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;

    const pkcs8 = new Uint8Array(await exportKey("pkcs8", orig.privateKey));
    const spki = new Uint8Array(await exportKey("spki", orig.publicKey));

    const priv = await importKey("pkcs8", pkcs8, { name }, true, ["sign"]);
    const pub = await importKey("spki", spki, { name }, true, ["verify"]);

    const msg = new TextEncoder().encode("via der");
    const sig = await crypto.subtle.sign({ name } as AnyAlg, priv, msg);
    const ok = await crypto.subtle.verify({ name } as AnyAlg, pub, sig, msg);
    assert(ok);
  });

  Deno.test(`webcrypto ${name} jwk round-trip + sign/verify`, async () => {
    const orig = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;

    // deno-lint-ignore no-explicit-any
    const privJwk: any = await (crypto.subtle.exportKey as AnyAlg)(
      "jwk",
      orig.privateKey,
    );
    assertEquals(privJwk.kty, "AKP");
    assertEquals(privJwk.alg, name);
    assert(typeof privJwk.pub === "string");
    assert(typeof privJwk.priv === "string");
    assertEquals(privJwk.key_ops, ["sign"]);
    assertEquals(privJwk.ext, true);

    // deno-lint-ignore no-explicit-any
    const pubJwk: any = await (crypto.subtle.exportKey as AnyAlg)(
      "jwk",
      orig.publicKey,
    );
    assertEquals(pubJwk.kty, "AKP");
    assertEquals(pubJwk.alg, name);
    assert(typeof pubJwk.pub === "string");
    assertEquals(pubJwk.priv, undefined);
    assertEquals(pubJwk.key_ops, ["verify"]);
    // The public key embedded in the private JWK matches the public JWK.
    assertEquals(privJwk.pub, pubJwk.pub);

    const priv = await importKey("jwk", privJwk, { name }, true, ["sign"]);
    const pub = await importKey("jwk", pubJwk, { name }, true, ["verify"]);
    assertEquals(priv.type, "private");
    assertEquals(pub.type, "public");

    const msg = new TextEncoder().encode("via jwk");
    const sig = await crypto.subtle.sign({ name } as AnyAlg, priv, msg);
    assert(await crypto.subtle.verify({ name } as AnyAlg, pub, sig, msg));

    // Re-export the re-imported private key and confirm the seed survives.
    // deno-lint-ignore no-explicit-any
    const privJwk2: any = await (crypto.subtle.exportKey as AnyAlg)(
      "jwk",
      priv,
    );
    assertEquals(privJwk2.priv, privJwk.priv);
    assertEquals(privJwk2.pub, privJwk.pub);
  });

  Deno.test(`webcrypto ${name} jwk export requires seed (raw-private)`, async () => {
    const orig = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;
    const rawPriv = new Uint8Array(
      await exportKey("raw-private", orig.privateKey),
    );
    const priv = await importKey(
      "raw-private",
      rawPriv,
      { name },
      true,
      ["sign"],
    );
    // No seed is available for a raw-private import, so JWK export must reject.
    await assertRejects(
      async () => {
        await (crypto.subtle.exportKey as AnyAlg)("jwk", priv);
      },
      DOMException,
    );
  });

  Deno.test(`webcrypto ${name} jwk import rejects mismatched pub`, async () => {
    const orig = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;
    // deno-lint-ignore no-explicit-any
    const privJwk: any = await (crypto.subtle.exportKey as AnyAlg)(
      "jwk",
      orig.privateKey,
    );
    const other = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;
    // deno-lint-ignore no-explicit-any
    const otherJwk: any = await (crypto.subtle.exportKey as AnyAlg)(
      "jwk",
      other.publicKey,
    );
    privJwk.pub = otherJwk.pub;
    await assertRejects(
      async () => {
        await importKey("jwk", privJwk, { name }, true, ["sign"]);
      },
      DOMException,
    );
  });

  Deno.test(`webcrypto ${name} getPublicKey() returns matching public key`, async () => {
    const { publicKey, privateKey } = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;

    const derived = (privateKey as AnyKey).getPublicKey();
    assert(derived);
    assertEquals(derived.type, "public");
    assertEquals(derived.algorithm.name, name);

    const rawPub = new Uint8Array(await exportKey("raw-public", publicKey));
    const rawDerived = new Uint8Array(await exportKey("raw-public", derived));
    assertEquals(rawDerived, rawPub);

    const data = new TextEncoder().encode("hi");
    const sig = await crypto.subtle.sign({ name } as AnyAlg, privateKey, data);
    assert(
      await crypto.subtle.verify({ name } as AnyAlg, derived, sig, data),
    );
  });

  Deno.test(`webcrypto ${name} getPublicKey() throws for public keys`, async () => {
    const { publicKey } = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;
    assertThrows(
      () => (publicKey as AnyKey).getPublicKey(),
      DOMException,
    );
  });

  Deno.test(`webcrypto ${name} signatures are non-deterministic`, async () => {
    const { privateKey } = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;
    const data = new TextEncoder().encode("nondeterministic");
    const sig1 = new Uint8Array(
      await crypto.subtle.sign({ name } as AnyAlg, privateKey, data),
    );
    const sig2 = new Uint8Array(
      await crypto.subtle.sign({ name } as AnyAlg, privateKey, data),
    );
    // ML-DSA "hedged" signing uses fresh randomness, so two signatures of
    // the same message must (with overwhelming probability) differ.
    assertNotEquals(sig1, sig2);
  });
}

Deno.test("webcrypto ML-DSA rejects invalid key usages", async () => {
  await assertRejects(
    async () => {
      await crypto.subtle.generateKey(
        { name: "ML-DSA-44" } as AnyAlg,
        true,
        ["encrypt"],
      );
    },
    DOMException,
  );
});

Deno.test("webcrypto ML-DSA import raw-seed rejects wrong length", async () => {
  await assertRejects(
    async () => {
      await importKey(
        "raw-seed",
        new Uint8Array(31),
        { name: "ML-DSA-44" },
        true,
        ["sign"],
      );
    },
    DOMException,
  );
});

Deno.test("webcrypto ML-DSA import raw-public rejects wrong length", async () => {
  await assertRejects(
    async () => {
      await importKey(
        "raw-public",
        new Uint8Array(64),
        { name: "ML-DSA-65" },
        true,
        ["verify"],
      );
    },
    DOMException,
  );
});

Deno.test("webcrypto ML-DSA spki round-trips for all variants", async () => {
  for (const name of ["ML-DSA-44", "ML-DSA-65", "ML-DSA-87"]) {
    const { publicKey } = await crypto.subtle.generateKey(
      { name } as AnyAlg,
      true,
      ["sign", "verify"],
    ) as CryptoKeyPair;
    const spki = new Uint8Array(await exportKey("spki", publicKey));
    const re = await importKey("spki", spki, { name }, true, ["verify"]);
    assertEquals(re.algorithm.name, name);
  }
});

Deno.test("webcrypto ML-DSA jwk import rejects wrong kty", async () => {
  const { publicKey } = await crypto.subtle.generateKey(
    { name: "ML-DSA-65" } as AnyAlg,
    true,
    ["sign", "verify"],
  ) as CryptoKeyPair;
  // deno-lint-ignore no-explicit-any
  const jwk: any = await (crypto.subtle.exportKey as AnyAlg)("jwk", publicKey);
  jwk.kty = "OKP";
  await assertRejects(
    async () => {
      await importKey("jwk", jwk, { name: "ML-DSA-65" }, true, ["verify"]);
    },
    DOMException,
  );
});

Deno.test("webcrypto ML-DSA jwk import rejects mismatched alg", async () => {
  const { publicKey } = await crypto.subtle.generateKey(
    { name: "ML-DSA-65" } as AnyAlg,
    true,
    ["sign", "verify"],
  ) as CryptoKeyPair;
  // deno-lint-ignore no-explicit-any
  const jwk: any = await (crypto.subtle.exportKey as AnyAlg)("jwk", publicKey);
  jwk.alg = "ML-DSA-87";
  await assertRejects(
    async () => {
      await importKey("jwk", jwk, { name: "ML-DSA-65" }, true, ["verify"]);
    },
    DOMException,
  );
});

Deno.test("webcrypto ML-DSA jwk import rejects bad private usage", async () => {
  const orig = await crypto.subtle.generateKey(
    { name: "ML-DSA-65" } as AnyAlg,
    true,
    ["sign", "verify"],
  ) as CryptoKeyPair;
  // deno-lint-ignore no-explicit-any
  const jwk: any = await (crypto.subtle.exportKey as AnyAlg)(
    "jwk",
    orig.privateKey,
  );
  // A private (priv present) JWK may only be imported for "sign".
  await assertRejects(
    async () => {
      await importKey("jwk", jwk, { name: "ML-DSA-65" }, true, ["verify"]);
    },
    DOMException,
  );
});
