// Copyright 2018-2026 the Deno authors. MIT license.
import crypto from "node:crypto";
import { Buffer } from "node:buffer";
import { Readable } from "node:stream";
import { buffer, text } from "node:stream/consumers";
import { assert, assertEquals, assertThrows } from "@std/assert";
import { AssertionError } from "node:assert";

const rsaPrivateKey = Deno.readTextFileSync(
  new URL("../testdata/rsa_private.pem", import.meta.url),
);
const rsaPublicKey = Deno.readTextFileSync(
  new URL("../testdata/rsa_public.pem", import.meta.url),
);

const input = Buffer.from("hello world", "utf-8");

function zeros(length: number): Uint8Array {
  return new Uint8Array(length);
}

Deno.test({
  name: "rsa public encrypt and private decrypt",
  fn() {
    const encrypted = crypto.publicEncrypt(Buffer.from(rsaPublicKey), input);
    const decrypted = crypto.privateDecrypt(
      Buffer.from(rsaPrivateKey),
      Buffer.from(encrypted),
    );
    assert(Buffer.isBuffer(encrypted));
    assert(Buffer.isBuffer(decrypted));
    assertEquals(decrypted, input);
  },
});

Deno.test({
  name: "rsa public encrypt (options) and private decrypt",
  fn() {
    const encrypted = crypto.publicEncrypt(
      { key: Buffer.from(rsaPublicKey) },
      input,
    );
    const decrypted = crypto.privateDecrypt(
      Buffer.from(rsaPrivateKey),
      Buffer.from(encrypted),
    );
    assertEquals(decrypted, input);
  },
});

Deno.test({
  name: "rsa private encrypt and public decrypt",
  fn() {
    const encrypted = crypto.privateEncrypt(rsaPrivateKey, input);
    assert(Buffer.isBuffer(encrypted));
    const decrypted = crypto.publicDecrypt(
      rsaPublicKey,
      Buffer.from(encrypted),
    );
    assert(Buffer.isBuffer(decrypted));
    assertEquals(decrypted, input);
  },
});

Deno.test({
  name: "encrypt decrypt with KeyObject",
  fn() {
    const pair = crypto.generateKeyPairSync("rsa", { modulusLength: 512 });
    const secret = Buffer.from("secret");
    const encrypted = crypto.publicEncrypt(pair.publicKey, secret);
    assert(Buffer.isBuffer(encrypted));
    const decrypted = crypto.privateDecrypt(pair.privateKey, encrypted);
    assert(Buffer.isBuffer(decrypted));
    assertEquals(decrypted, secret);
  },
});

Deno.test({
  name: "rsa public decrypt fail",
  fn() {
    const encrypted = crypto.publicEncrypt(rsaPublicKey, input);
    assert(Buffer.isBuffer(encrypted));
    assertThrows(() =>
      crypto.publicDecrypt(rsaPublicKey, Buffer.from(encrypted))
    );
  },
});

Deno.test({
  name: "createCipheriv - multiple chunk inputs",
  fn() {
    const cipher = crypto.createCipheriv(
      "aes-128-cbc",
      new Uint8Array(16),
      new Uint8Array(16),
    );
    assertEquals(
      cipher.update(new Uint8Array(16), undefined, "hex"),
      "66e94bd4ef8a2c3b884cfa59ca342b2e",
    );
    assertEquals(
      cipher.update(new Uint8Array(19), undefined, "hex"),
      "f795bd4a52e29ed713d313fa20e98dbc",
    );
    assertEquals(
      cipher.update(new Uint8Array(55), undefined, "hex"),
      "a10cf66d0fddf3405370b4bf8df5bfb347c78395e0d8ae2194da0a90abc9888a94ee48f6c78fcd518a941c3896102cb1",
    );
    assertEquals(cipher.final("hex"), "e11901dde4a2f99fe4efc707e48c6aed");
  },
});

Deno.test({
  name: "createCipheriv - algorithms",
  fn() {
    const table = [
      [
        ["aes-128-cbc", 16, 16],
        "66e94bd4ef8a2c3b884cfa59ca342b2ef795bd4a52e29ed713d313fa20e98dbca10cf66d0fddf3405370b4bf8df5bfb3",
        "d5f65ecda64511e9d3d12206411ffd72",
      ],
      [
        ["aes-128-ecb", 16, 0],
        "66e94bd4ef8a2c3b884cfa59ca342b2e66e94bd4ef8a2c3b884cfa59ca342b2e66e94bd4ef8a2c3b884cfa59ca342b2e",
        "baf823258ca2e6994f638daa3515e986",
      ],
      [
        ["aes-192-ecb", 24, 0],
        "aae06992acbf52a3e8f4a96ec9300bd7aae06992acbf52a3e8f4a96ec9300bd7aae06992acbf52a3e8f4a96ec9300bd7",
        "2e0f33b51bb184654311ead507ea55fc",
      ],
      [
        ["aes-256-ecb", 32, 0],
        "dc95c078a2408989ad48a21492842087dc95c078a2408989ad48a21492842087dc95c078a2408989ad48a21492842087",
        "0ac1d7e8655254c6814b46753932df88",
      ],
      [
        ["aes256", 32, 16],
        "dc95c078a2408989ad48a2149284208708c374848c228233c2b34f332bd2e9d38b70c515a6663d38cdb8e6532b266491",
        "2e62607a5e8b715e4cb229a12169f2b2",
      ],
      [
        ["aes-256-cbc", 32, 16],
        "dc95c078a2408989ad48a2149284208708c374848c228233c2b34f332bd2e9d38b70c515a6663d38cdb8e6532b266491",
        "2e62607a5e8b715e4cb229a12169f2b2",
      ],
      [
        ["aes-128-ctr", 16, 16],
        "66e94bd4ef8a2c3b884cfa59ca342b2e58e2fccefa7e3061367f1d57a4e7455a0388dace60b6a392f328c2b971b2fe78f795",
        "",
      ],
      [
        ["aes-192-ctr", 24, 16],
        "aae06992acbf52a3e8f4a96ec9300bd7cd33b28ac773f74ba00ed1f31257243598e7247c07f0fe411c267e4384b0f6002a34",
        "",
      ],
      [
        ["aes-256-ctr", 32, 16],
        "dc95c078a2408989ad48a21492842087530f8afbc74536b9a963b4f1c4cb738bcea7403d4d606b6e074ec5d3baf39d187260",
        "",
      ],
    ] as const;
    for (
      const [[alg, keyLen, ivLen], expectedUpdate, expectedFinal] of table
    ) {
      const cipher = crypto.createCipheriv(alg, zeros(keyLen), zeros(ivLen));
      assertEquals(cipher.update(zeros(50), undefined, "hex"), expectedUpdate);
      assertEquals(cipher.final("hex"), expectedFinal);
    }
  },
});

Deno.test({
  name: "createCipheriv - input encoding",
  fn() {
    {
      const cipher = crypto.createCipheriv(
        "aes-128-cbc",
        new Uint8Array(16),
        new Uint8Array(16),
      );
      assertEquals(
        cipher.update("hello, world! hello, world!", "utf-8", "hex"),
        "ca7df4d74f51b77a7440ead38343ab0f",
      );
      assertEquals(cipher.final("hex"), "d0da733dec1fa61125c80a6f97e6166e");
    }

    {
      const cipher = crypto.createCipheriv(
        "aes-128-cbc",
        new Uint8Array(16),
        new Uint8Array(16),
      );
      // update with string without input encoding
      assertEquals(
        cipher.update("hello, world! hello, world!").toString("hex"),
        "ca7df4d74f51b77a7440ead38343ab0f",
      );
      assertEquals(cipher.final("hex"), "d0da733dec1fa61125c80a6f97e6166e");
    }
  },
});

Deno.test({
  name: "createCipheriv - transform stream",
  async fn() {
    const result = await buffer(
      Readable.from("foo".repeat(15)).pipe(crypto.createCipheriv(
        "aes-128-cbc",
        new Uint8Array(16),
        new Uint8Array(16),
      )),
    );
    // deno-fmt-ignore
    assertEquals([...result], [
      129,  19, 202, 142, 137,  51,  23,  53, 198,  33,
      214, 125,  17,   5, 128,  57, 162, 217, 220,  53,
      172,  51,  85, 113,  71, 250,  44, 156,  80,   4,
      158,  92, 185, 173,  67,  47, 255,  71,  78, 187,
       80, 206,  42,   5,  34, 104,   1,  54
    ]);
  },
});

Deno.test({
  name: "createDecipheriv - algorithms",
  fn() {
    const table = [
      [
        ["aes-128-cbc", 16, 16],
        "66e94bd4ef8a2c3b884cfa59ca342b2ef795bd4a52e29ed713d313fa20e98dbca10cf66d0fddf3405370b4bf8df5bfb347c78395e0d8ae2194da0a90abc9888a94ee48f6c78fcd518a941c3896102cb1e11901dde4a2f99fe4efc707e48c6aed",
      ],
      [
        ["aes-128-ecb", 16, 0],
        "66e94bd4ef8a2c3b884cfa59ca342b2e66e94bd4ef8a2c3b884cfa59ca342b2e66e94bd4ef8a2c3b884cfa59ca342b2e66e94bd4ef8a2c3b884cfa59ca342b2e66e94bd4ef8a2c3b884cfa59ca342b2ec29a917cbaf72fa9bc32129bb0d17663",
      ],
      [
        ["aes-192-ecb", 24, 0],
        "aae06992acbf52a3e8f4a96ec9300bd7aae06992acbf52a3e8f4a96ec9300bd7aae06992acbf52a3e8f4a96ec9300bd7aae06992acbf52a3e8f4a96ec9300bd7aae06992acbf52a3e8f4a96ec9300bd7ab40eb56b6fc2aacf2e9254685cce891",
      ],
      [
        ["aes-256-ecb", 32, 0],
        "dc95c078a2408989ad48a21492842087dc95c078a2408989ad48a21492842087dc95c078a2408989ad48a21492842087dc95c078a2408989ad48a21492842087dc95c078a2408989ad48a214928420877c45b49560579dd1ffc7ec626de2a968",
      ],
      [
        ["aes256", 32, 16],
        "dc95c078a2408989ad48a2149284208708c374848c228233c2b34f332bd2e9d38b70c515a6663d38cdb8e6532b2664915d0dcc192580aee9ef8a8568193f1b44bfca557c6bab7dc79da07ffd42191b2659e6bee99cb2a6a7299f0e9a21686fc7",
      ],
      [
        ["aes-256-cbc", 32, 16],
        "dc95c078a2408989ad48a2149284208708c374848c228233c2b34f332bd2e9d38b70c515a6663d38cdb8e6532b2664915d0dcc192580aee9ef8a8568193f1b44bfca557c6bab7dc79da07ffd42191b2659e6bee99cb2a6a7299f0e9a21686fc7",
      ],
      [
        ["aes-128-ctr", 16, 16],
        "66e94bd4ef8a2c3b884cfa59ca342b2e58e2fccefa7e3061367f1d57a4e7455a0388dace60b6a392f328c2b971b2fe78f795aaab494b5923f7fd89ff948bc1e0200211214e7394da2089b6acd093abe0",
        Buffer.alloc(0),
      ],
      [
        ["aes-192-ctr", 24, 16],
        "aae06992acbf52a3e8f4a96ec9300bd7cd33b28ac773f74ba00ed1f31257243598e7247c07f0fe411c267e4384b0f6002a3493e66235ee67deeccd2f3b393bd8fdaa17c2cde20268fe36e164ea532151",
        Buffer.alloc(0),
      ],
      [
        ["aes-256-ctr", 32, 16],
        "dc95c078a2408989ad48a21492842087530f8afbc74536b9a963b4f1c4cb738bcea7403d4d606b6e074ec5d3baf39d18726003ca37a62a74d1a2f58e7506358edd4ab1284d4ae17b41e85924470c36f7",
        Buffer.alloc(0),
      ],
    ] as const;
    for (
      const [[alg, keyLen, ivLen], input, final] of table
    ) {
      const cipher = crypto.createDecipheriv(alg, zeros(keyLen), zeros(ivLen));
      assertEquals(cipher.update(input, "hex"), Buffer.alloc(80));
      assertEquals(cipher.final(), final ?? Buffer.alloc(10));
    }
  },
});

Deno.test({
  name: "createDecipheriv - transform stream",
  async fn() {
    const stream = Readable.from([
      // deno-fmt-ignore
      new Uint8Array([
        129,  19, 202, 142, 137,  51,  23,  53, 198,  33,
        214, 125,  17,   5, 128,  57, 162, 217, 220,  53,
        172,  51,  85, 113,  71, 250,  44, 156,  80,   4,
        158,  92, 185, 173,  67,  47, 255,  71,  78, 187,
         80, 206,  42,   5,  34, 104,   1,  54
      ]),
    ]).pipe(crypto.createDecipheriv(
      "aes-128-cbc",
      new Uint8Array(16),
      new Uint8Array(16),
    ));
    assertEquals(await text(stream), "foo".repeat(15));
  },
});

Deno.test({
  name: "createCipheriv - invalid algorithm",
  fn() {
    assertThrows(
      () =>
        crypto.createCipheriv("foo", new Uint8Array(16), new Uint8Array(16)),
      Error,
      "Unknown cipher",
    );
  },
});

Deno.test({
  name: "createCipheriv - invalid inputs",
  fn() {
    const enum Invalid {
      Key,
      Iv,
    }
    const table = [
      ["aes128", 15, 16, Invalid.Key],
      ["aes-128-cbc", 15, 16, Invalid.Key],
      ["aes128", 16, 15, Invalid.Iv],
      ["aes-128-cbc", 16, 15, Invalid.Iv],
      ["aes256", 31, 16, Invalid.Key],
      ["aes-256-cbc", 31, 16, Invalid.Key],
      ["aes256", 32, 15, Invalid.Iv],
      ["aes-256-cbc", 32, 15, Invalid.Iv],
      ["aes-128-ecb", 15, 0, Invalid.Key],
      ["aes-192-ecb", 16, 0, Invalid.Key],
      ["aes-256-ecb", 16, 0, Invalid.Key],
      ["aes-128-ctr", 32, 16, Invalid.Key],
      ["aes-128-ctr", 16, 32, Invalid.Iv],
      ["aes-192-ctr", 16, 16, Invalid.Key],
      ["aes-192-ctr", 24, 32, Invalid.Iv],
      ["aes-256-ctr", 16, 16, Invalid.Key],
      ["aes-256-ctr", 32, 32, Invalid.Iv],
    ] as const;
    for (const [algorithm, keyLen, ivLen, invalid] of table) {
      assertThrows(
        () =>
          crypto.createCipheriv(
            algorithm,
            new Uint8Array(keyLen),
            new Uint8Array(ivLen),
          ),
        invalid === Invalid.Key ? RangeError : TypeError,
        invalid === Invalid.Key
          ? "Invalid key length"
          : "Invalid initialization vector",
      );
    }
  },
});

Deno.test({
  name: "createDecipheriv - invalid algorithm",
  fn() {
    assertThrows(
      () =>
        crypto.createDecipheriv("foo", new Uint8Array(16), new Uint8Array(16)),
      Error,
      "Unknown cipher",
    );
  },
});

Deno.test({
  name: "createDecipheriv - invalid inputs",
  fn() {
    const enum Invalid {
      Key,
      Iv,
    }
    const table = [
      ["aes-128-cbc", 15, 16, Invalid.Key],
      ["aes-128-cbc", 16, 15, Invalid.Iv],
      ["aes256", 31, 16, Invalid.Key],
      ["aes-256-cbc", 31, 16, Invalid.Key],
      ["aes256", 32, 15, Invalid.Iv],
      ["aes-256-cbc", 32, 15, Invalid.Iv],
      ["aes-128-ecb", 15, 0, Invalid.Key],
      ["aes-192-ecb", 16, 0, Invalid.Key],
      ["aes-256-ecb", 16, 0, Invalid.Key],
      ["aes-128-ctr", 32, 16, Invalid.Key],
      ["aes-128-ctr", 16, 32, Invalid.Iv],
      ["aes-192-ctr", 16, 16, Invalid.Key],
      ["aes-192-ctr", 24, 32, Invalid.Iv],
      ["aes-256-ctr", 16, 16, Invalid.Key],
      ["aes-256-ctr", 32, 32, Invalid.Iv],
    ] as const;
    for (const [algorithm, keyLen, ivLen, invalid] of table) {
      assertThrows(
        () =>
          crypto.createDecipheriv(
            algorithm,
            new Uint8Array(keyLen),
            new Uint8Array(ivLen),
          ),
        invalid === Invalid.Key ? RangeError : TypeError,
        invalid === Invalid.Key
          ? "Invalid key length"
          : "Invalid initialization vector",
      );
    }
  },
});

Deno.test({
  name: "getCiphers",
  fn() {
    assertEquals(crypto.getCiphers().includes("aes-128-cbc"), true);
    assertEquals(crypto.getCiphers().includes("aes-256-ctr"), true);

    const getZeroKey = (cipher: string) => {
      if (cipher === "des-ede3-cbc") return zeros(24);
      if (cipher === "chacha20" || cipher === "chacha20-poly1305") {
        return zeros(32);
      }
      return zeros(+cipher.match(/\d+/)![0] / 8);
    };
    const getZeroIv = (cipher: string) => {
      // ECB mode takes no IV; a non-empty IV is now rejected.
      if (cipher.includes("ecb")) {
        return zeros(0);
      }
      if (cipher.includes("gcm")) {
        return zeros(12);
      }
      if (cipher === "chacha20-poly1305") return zeros(12);
      if (cipher.includes("des")) return zeros(8);
      return zeros(16);
    };

    for (const cipher of crypto.getCiphers()) {
      crypto.createCipheriv(cipher, getZeroKey(cipher), getZeroIv(cipher))
        .final();
    }
  },
});

Deno.test({
  name: "getCipherInfo",
  fn() {
    const info = crypto.getCipherInfo("aes-128-cbc")!;
    assertEquals(info.name, "aes-128-cbc");
    assertEquals(info.keyLength, 16);
    assertEquals(info.ivLength, 16);

    const info2 = crypto.getCipherInfo("aes128")!;
    assertEquals(info2.name, "aes-128-cbc");
    assertEquals(info2.keyLength, 16);
    assertEquals(info2.ivLength, 16);
  },
});

Deno.test({
  name:
    "createDecipheriv - handling of the last chunk when auto padding enabled/disabled",
  fn() {
    const algorithm = "aes-256-cbc";
    const key = Buffer.from(
      "84dcdd964968734fdf0de4a2cba471c2e0a753930b841c014b1e77f456b5797b",
      "hex",
    );
    const val = Buffer.from(
      "feabbdf66e2c71cc780d0cd2765dcce283e8ae7e58fcc1a9acafc678581e0e06",
      "hex",
    );
    const iv = Buffer.alloc(16, 0);

    {
      const decipher = crypto.createDecipheriv(algorithm, key, iv);
      decipher.setAutoPadding(false);
      assertEquals(
        decipher.update(val, undefined, "hex"),
        "ed2c908f26571bf8e50d60b77fb9c25f95b933b59111543c6fac41ad6b47e681",
      );
      assertEquals(decipher.final("hex"), "");
    }

    {
      const decipher = crypto.createDecipheriv(algorithm, key, iv);
      assertEquals(
        decipher.update(val, undefined, "hex"),
        "ed2c908f26571bf8e50d60b77fb9c25f",
      );
      assertThrows(() => {
        decipher.final();
      });
    }
  },
});

Deno.test({
  name: "createDecipheriv - invalid final block len",
  fn() {
    const algorithm = "aes-256-cbc";
    const key = Buffer.from(
      "84dcdd964968734fdf0de4a2cba471c2e0a753930b841c014b1e77f456b5797b",
      "hex",
    );
    const iv = Buffer.alloc(16, 0);

    const decipher = crypto.createDecipheriv(algorithm, key, iv);
    decipher.update(Buffer.alloc(12));
    assertThrows(
      () => {
        decipher.final();
      },
      RangeError,
      "wrong final block length",
    );
  },
});

Deno.test({
  name: "Cipheriv - change encoding after first update",
  fn() {
    const cipher = crypto.createCipheriv(
      "aes-128-cbc",
      new Uint8Array(16),
      new Uint8Array(16),
    );
    cipher.update(new Uint8Array(16), undefined, "hex");
    assertThrows(
      () => {
        cipher.final("utf-8");
      },
      AssertionError,
      "Cannot change encoding",
    );

    cipher.final();
  },
});

Deno.test({
  name: "Decipheriv - change encoding after first update",
  fn() {
    const decipher = crypto.createDecipheriv(
      "aes-256-cbc",
      new Uint8Array(32),
      new Uint8Array(16),
    );
    decipher.update(new Uint32Array(), undefined, "hex");
    assertThrows(
      () => {
        decipher.final("utf-8");
      },
      AssertionError,
      "Cannot change encoding",
    );

    decipher.final();
  },
});

// https://github.com/denoland/deno/issues/30722
Deno.test({
  name: "base64 cipher/decipher with non multiple of 4 char length",
  fn() {
    const aes256Encrypt = (plaintext: string, key: string) => {
      const hash = crypto.createHash("sha256").update(key).digest();
      const iv = crypto.randomBytes(16);
      const cipher = crypto.createCipheriv("aes-256-cbc", hash, iv);

      let encrypted = cipher.update(plaintext, "utf8", "base64");
      encrypted += cipher.final("base64");

      return iv.toString("base64") + "." + encrypted;
    };

    const aes256Decrypt = (encryptedText: string, key: string): string => {
      const hash = crypto.createHash("sha256").update(key).digest();
      const [ivPart, encryptedPart] = encryptedText.split(".");

      const iv = Buffer.from(ivPart, "base64");
      const decipher = crypto.createDecipheriv("aes-256-cbc", hash, iv);

      let decrypted = decipher.update(encryptedPart, "base64", "utf8");
      decrypted += decipher.final("utf8");

      return decrypted;
    };

    const key = "my secret key";
    const text = "The quick brown fox jumps over the lazy dog";

    const cipherText = aes256Encrypt(text, key);
    const decryptedText = aes256Decrypt(cipherText, key);

    assertEquals(decryptedText, text);
  },
});

Deno.test({
  name: "createCipheriv - setAutoPadding behavior",
  fn() {
    const algorithm = "aes-256-cbc";
    const key = Buffer.alloc(32, 0);
    const iv = Buffer.alloc(16, 0);

    const cipherWithAutoPadding = crypto
      .createCipheriv(algorithm, key, iv)
      .setAutoPadding(true);
    let encrypted = cipherWithAutoPadding.update("", "utf8", "binary");
    encrypted += cipherWithAutoPadding.final("binary");
    assertEquals(encrypted.length, 16);

    const cipherWithoutAutoPadding = crypto
      .createCipheriv(algorithm, key, iv)
      .setAutoPadding(false);
    let otherEncrypted = cipherWithoutAutoPadding.update("", "utf8", "binary");
    otherEncrypted += cipherWithoutAutoPadding.final("binary");
    assertEquals(otherEncrypted.length, 0);
  },
});

Deno.test({
  name: "createCipheriv - cipher lockdown after final()",
  fn() {
    const key = crypto.randomBytes(32);
    const iv = crypto.randomBytes(16);
    const cipher = crypto.createCipheriv("aes-256-cbc", key, iv);

    // Call final() to lock down the cipher
    cipher.final();

    assertThrows(
      () => {
        cipher.update("test data");
      },
      Error,
      "Invalid state for operation update",
    );

    assertThrows(
      () => {
        cipher.final();
      },
      Error,
      "Invalid state for operation final",
    );
  },
});

Deno.test({
  name: "createDecipheriv - decipher lockdown after final()",
  fn() {
    const key = crypto.randomBytes(32);
    const iv = crypto.randomBytes(16);

    const cipher = crypto.createCipheriv("aes-256-cbc", key, iv);
    const encrypted = Buffer.concat([
      cipher.update("test data"),
      cipher.final(),
    ]);

    const decipher = crypto.createDecipheriv("aes-256-cbc", key, iv);
    decipher.update(encrypted);
    decipher.final();

    assertThrows(
      () => {
        decipher.update(encrypted);
      },
      Error,
      "Invalid state for operation update",
    );

    assertThrows(
      () => {
        decipher.final();
      },
      Error,
      "Invalid state for operation final",
    );
  },
});

Deno.test({
  name: "aes-256-cbc setAutoPadding(false) roundtrip",
  fn() {
    const key = Buffer.alloc(32, 0x01);
    const iv = Buffer.alloc(16, 0x02);
    // 32 bytes = exactly 2 blocks, no PKCS7 padding needed
    const plaintext = Buffer.from("a]B$y;^&5}0[e-k+D7zIn9Co*q#m!LpX");

    const cipher = crypto.createCipheriv("aes-256-cbc", key, iv);
    cipher.setAutoPadding(false);
    const encrypted = Buffer.concat([cipher.update(plaintext), cipher.final()]);

    const decipher = crypto.createDecipheriv("aes-256-cbc", key, iv);
    decipher.setAutoPadding(false);
    const decrypted = Buffer.concat([
      decipher.update(encrypted),
      decipher.final(),
    ]);

    assertEquals(decrypted, plaintext);
  },
});

Deno.test({
  name: "publicEncrypt/privateDecrypt with DER keys",
  fn() {
    const pair = crypto.generateKeyPairSync("rsa", { modulusLength: 2048 });

    // Export as DER (binary) format
    const publicDer = pair.publicKey.export({ type: "spki", format: "der" });
    const privateDer = pair.privateKey.export({
      type: "pkcs8",
      format: "der",
    });

    const secret = Buffer.from("hello DER keys");
    const encrypted = crypto.publicEncrypt(
      { key: publicDer, padding: crypto.constants.RSA_PKCS1_PADDING },
      secret,
    );
    assert(Buffer.isBuffer(encrypted));

    const decrypted = crypto.privateDecrypt(
      { key: privateDer, padding: crypto.constants.RSA_PKCS1_PADDING },
      encrypted,
    );
    assertEquals(decrypted, secret);
  },
});

Deno.test({
  name: "aes-128-cbc setAutoPadding(false) roundtrip",
  fn() {
    const key = Buffer.alloc(16, 0x03);
    const iv = Buffer.alloc(16, 0x04);
    // 16 bytes = exactly 1 block
    const plaintext = Buffer.from("0123456789abcdef");

    const cipher = crypto.createCipheriv("aes-128-cbc", key, iv);
    cipher.setAutoPadding(false);
    const encrypted = Buffer.concat([cipher.update(plaintext), cipher.final()]);

    const decipher = crypto.createDecipheriv("aes-128-cbc", key, iv);
    decipher.setAutoPadding(false);
    const decrypted = Buffer.concat([
      decipher.update(encrypted),
      decipher.final(),
    ]);

    assertEquals(decrypted, plaintext);
  },
});

Deno.test({
  name: "publicEncrypt/privateDecrypt with PKCS#1 DER keys",
  fn() {
    const pair = crypto.generateKeyPairSync("rsa", { modulusLength: 2048 });

    // Export as PKCS#1 DER (binary) format
    const publicDer = pair.publicKey.export({ type: "pkcs1", format: "der" });
    const privateDer = pair.privateKey.export({
      type: "pkcs1",
      format: "der",
    });

    const secret = Buffer.from("hello PKCS1 DER keys");
    const encrypted = crypto.publicEncrypt(
      { key: publicDer, padding: crypto.constants.RSA_PKCS1_PADDING },
      secret,
    );
    assert(Buffer.isBuffer(encrypted));

    const decrypted = crypto.privateDecrypt(
      { key: privateDer, padding: crypto.constants.RSA_PKCS1_PADDING },
      encrypted,
    );
    assertEquals(decrypted, secret);
  },
});

// Regression test for https://github.com/denoland/deno/issues/31957
Deno.test({
  name: "createDecipheriv - setAutoPadding(false) with empty final input",
  fn() {
    // Test aes-256-ecb (the original issue)
    {
      const decipher = crypto.createDecipheriv(
        "aes-256-ecb",
        Buffer.alloc(32),
        "",
      );
      decipher.setAutoPadding(false);
      const output = decipher.update(Buffer.alloc(16));
      assertEquals(output.length, 16);
      decipher.final();
    }

    // Test aes-128-ecb
    {
      const decipher = crypto.createDecipheriv(
        "aes-128-ecb",
        Buffer.alloc(16),
        "",
      );
      decipher.setAutoPadding(false);
      const output = decipher.update(Buffer.alloc(16));
      assertEquals(output.length, 16);
      decipher.final();
    }

    // Test aes-192-ecb
    {
      const decipher = crypto.createDecipheriv(
        "aes-192-ecb",
        Buffer.alloc(24),
        "",
      );
      decipher.setAutoPadding(false);
      const output = decipher.update(Buffer.alloc(16));
      assertEquals(output.length, 16);
      decipher.final();
    }

    // Test aes-128-cbc
    {
      const decipher = crypto.createDecipheriv(
        "aes-128-cbc",
        Buffer.alloc(16),
        Buffer.alloc(16),
      );
      decipher.setAutoPadding(false);
      const output = decipher.update(Buffer.alloc(16));
      assertEquals(output.length, 16);
      decipher.final();
    }

    // Test aes-256-cbc
    {
      const decipher = crypto.createDecipheriv(
        "aes-256-cbc",
        Buffer.alloc(32),
        Buffer.alloc(16),
      );
      decipher.setAutoPadding(false);
      const output = decipher.update(Buffer.alloc(16));
      assertEquals(output.length, 16);
      decipher.final();
    }
  },
});

Deno.test({
  name:
    "createDecipheriv - setAutoPadding(false) with invalid block length should error",
  fn() {
    // Invalid block length (10 bytes instead of 16) should throw an error, not panic
    // Test all affected cipher modes

    // aes-256-ecb
    {
      const decipher = crypto.createDecipheriv(
        "aes-256-ecb",
        Buffer.alloc(32),
        "",
      );
      decipher.setAutoPadding(false);
      decipher.update(Buffer.alloc(10));
      assertThrows(
        () => {
          decipher.final();
        },
        RangeError,
        "wrong final block length",
      );
    }

    // aes-128-ecb
    {
      const decipher = crypto.createDecipheriv(
        "aes-128-ecb",
        Buffer.alloc(16),
        "",
      );
      decipher.setAutoPadding(false);
      decipher.update(Buffer.alloc(10));
      assertThrows(
        () => {
          decipher.final();
        },
        RangeError,
        "wrong final block length",
      );
    }

    // aes-192-ecb
    {
      const decipher = crypto.createDecipheriv(
        "aes-192-ecb",
        Buffer.alloc(24),
        "",
      );
      decipher.setAutoPadding(false);
      decipher.update(Buffer.alloc(10));
      assertThrows(
        () => {
          decipher.final();
        },
        RangeError,
        "wrong final block length",
      );
    }

    // aes-128-cbc
    {
      const decipher = crypto.createDecipheriv(
        "aes-128-cbc",
        Buffer.alloc(16),
        Buffer.alloc(16),
      );
      decipher.setAutoPadding(false);
      decipher.update(Buffer.alloc(10));
      assertThrows(
        () => {
          decipher.final();
        },
        RangeError,
        "wrong final block length",
      );
    }

    // aes-256-cbc
    {
      const decipher = crypto.createDecipheriv(
        "aes-256-cbc",
        Buffer.alloc(32),
        Buffer.alloc(16),
      );
      decipher.setAutoPadding(false);
      decipher.update(Buffer.alloc(10));
      assertThrows(
        () => {
          decipher.final();
        },
        RangeError,
        "wrong final block length",
      );
    }
  },
});

Deno.test({
  name: "createDecipheriv - invalid PKCS7 padding throws bad decrypt",
  fn() {
    const key = Buffer.alloc(16, 0x01);
    const iv = Buffer.alloc(16, 0x02);

    // Encrypt without padding so we control the raw plaintext bytes
    const cipher = crypto.createCipheriv("aes-128-cbc", key, iv);
    cipher.setAutoPadding(false);

    // Last byte 0x00 is invalid PKCS7 padding (valid range is 1-16)
    const plaintext = Buffer.from(
      "0123456789abcde\x00",
    );
    const encrypted = Buffer.concat([
      cipher.update(plaintext),
      cipher.final(),
    ]);

    // Decrypt with autoPadding=true (default) should throw
    const decipher = crypto.createDecipheriv("aes-128-cbc", key, iv);
    assertThrows(
      () => {
        decipher.update(encrypted);
        decipher.final();
      },
      Error,
      "bad decrypt",
    );
  },
});

Deno.test({
  name: "chacha20-poly1305 repeated setAAD produces correct tag",
  fn() {
    const key = Buffer.alloc(32, 0x42);
    const iv = Buffer.alloc(12, 0x24);
    const plaintext = Buffer.from("hello world");
    const aad1 = Buffer.from("first");
    const aad2 = Buffer.from("second");
    const fullAad = Buffer.concat([aad1, aad2]);

    // deno-lint-ignore no-explicit-any
    const createCipher = (crypto.createCipheriv as any).bind(crypto);
    // deno-lint-ignore no-explicit-any
    const createDecipher = (crypto.createDecipheriv as any).bind(crypto);
    const opts = { authTagLength: 16 };

    // Encrypt with single setAAD containing the full AAD
    const c1 = createCipher("chacha20-poly1305", key, iv, opts);
    c1.setAAD(fullAad);
    const enc1 = Buffer.concat([c1.update(plaintext), c1.final()]);
    const tag1 = c1.getAuthTag();

    // Encrypt with two separate setAAD calls
    const c2 = createCipher("chacha20-poly1305", key, iv, opts);
    c2.setAAD(aad1);
    c2.setAAD(aad2);
    const enc2 = Buffer.concat([c2.update(plaintext), c2.final()]);
    const tag2 = c2.getAuthTag();

    assertEquals(enc1, enc2);
    assertEquals(tag1, tag2);

    // Verify decryption with combined AAD works for both
    const d = createDecipher("chacha20-poly1305", key, iv, opts);
    d.setAAD(fullAad);
    d.setAuthTag(tag2);
    const dec = Buffer.concat([d.update(enc2), d.final()]);
    assertEquals(dec, plaintext);
  },
});

Deno.test({
  name: "chacha20 matches the RFC 8439 test vector and round-trips",
  fn() {
    // RFC 8439 §2.4.2. Node/OpenSSL's chacha20 IV is the 4-byte
    // little-endian initial block counter (1 here) followed by the
    // 12-byte nonce.
    const key = Buffer.from(
      "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      "hex",
    );
    const iv = Buffer.from("01000000000000000000004a00000000", "hex");
    const plaintext = "Ladies and Gentlemen of the class of '99: " +
      "If I could offer you only one tip for the future, " +
      "sunscreen would be it.";
    const expected = "6e2e359a2568f98041ba0728dd0d6981e97e7aec1d4360c20a27af" +
      "ccfd9fae0bf91b65c5524733ab8f593dabcd62b3571639d624e65152ab8f530c359f" +
      "0861d807ca0dbf500d6a6156a38e088a22b65e52bc514d16ccf806818ce91ab77937" +
      "365af90bbf74a35be6b40b8eedf2785e42874d";

    const cipher = crypto.createCipheriv("chacha20", key, iv);
    const encrypted = Buffer.concat([
      cipher.update(plaintext, "utf8"),
      cipher.final(),
    ]);
    assertEquals(encrypted.toString("hex"), expected);

    const decipher = crypto.createDecipheriv("chacha20", key, iv);
    const decrypted = Buffer.concat([
      decipher.update(encrypted),
      decipher.final(),
    ]);
    assertEquals(decrypted.toString("utf8"), plaintext);
  },
});

Deno.test({
  name: "chacha20 keeps keystream position across chunked updates",
  fn() {
    const key = Buffer.alloc(32, 1);
    // Counter bytes are 0x02020202, exercising a non-zero initial counter.
    const iv = Buffer.alloc(16, 2);
    const plaintext = Buffer.from(
      "hello world, this is a longer test message to cross the 64-byte " +
        "chacha block boundary!!",
    );
    // Expected ciphertext produced by Node.js v24.15.0.
    const expected = "788127700131850a5c17dbe2f7bb9664b4fe2c20380c8065b37b9d" +
      "11deab2d0e84cd61454063883ed1fe8497ed5543b813fdebc64b37d9dd192dcd7202" +
      "daf0d393c2503713e0f3eaeff34109bc1a861ab0d213f61d1216";

    const oneShot = crypto.createCipheriv("chacha20", key, iv);
    const encrypted = Buffer.concat([
      oneShot.update(plaintext),
      oneShot.final(),
    ]);
    assertEquals(encrypted.toString("hex"), expected);

    // Feed the same plaintext in chunks that are not multiples of the
    // 64-byte ChaCha block so updates start mid-block.
    const chunked = crypto.createCipheriv("chacha20", key, iv);
    const outputs: Buffer[] = [];
    for (const [start, end] of [[0, 1], [1, 7], [7, 70], [70, undefined]]) {
      outputs.push(chunked.update(plaintext.subarray(start, end)));
    }
    outputs.push(chunked.final());
    assertEquals(Buffer.concat(outputs).toString("hex"), expected);

    // Single updates large enough to consume leftover keystream, cross
    // several whole blocks, and end mid-block again, all in one call.
    const long = Buffer.alloc(300);
    for (let i = 0; i < long.length; i++) long[i] = i & 0xff;
    const longOneShot = crypto.createCipheriv("chacha20", key, iv);
    const longExpected = Buffer.concat([
      longOneShot.update(long),
      longOneShot.final(),
    ]);
    const longChunked = crypto.createCipheriv("chacha20", key, iv);
    const longOutputs: Buffer[] = [];
    for (const [start, end] of [[0, 1], [1, 131], [131, undefined]]) {
      longOutputs.push(longChunked.update(long.subarray(start, end)));
    }
    longOutputs.push(longChunked.final());
    assertEquals(
      Buffer.concat(longOutputs).toString("hex"),
      longExpected.toString("hex"),
    );
  },
});

Deno.test({
  name: "chacha20 rejects invalid key and iv lengths",
  fn() {
    for (const keyLen of [16, 31, 33]) {
      assertThrows(
        () => crypto.createCipheriv("chacha20", zeros(keyLen), zeros(16)),
        RangeError,
        "Invalid key length",
      );
    }
    for (const ivLen of [0, 12, 17]) {
      assertThrows(
        () => crypto.createCipheriv("chacha20", zeros(32), zeros(ivLen)),
        TypeError,
        "Invalid initialization vector",
      );
    }
  },
});

Deno.test({
  name: "chacha20 is listed in getCiphers and getCipherInfo",
  fn() {
    assert(crypto.getCiphers().includes("chacha20"));
    const info = crypto.getCipherInfo("chacha20")!;
    assertEquals(info.name, "chacha20");
    assertEquals(info.nid, 1019);
    assertEquals(info.keyLength, 32);
    assertEquals(info.ivLength, 16);
    assertEquals(info.mode, "stream");
  },
});

// Helper for the tests below: assert that a cipher/decipher created with
// `algorithm` rejects each `invalidValue` from `update()` with a TypeError
// whose code is ERR_INVALID_ARG_TYPE, and releases the native resource.
function assertUpdateRejects(
  algorithm: string,
  keyLen: number,
  ivLen: number,
  invalidValues: readonly unknown[],
) {
  const key = Buffer.alloc(keyLen);
  const iv = ivLen === 0 ? null : Buffer.alloc(ivLen);

  for (const value of invalidValues) {
    for (
      const factory of [crypto.createCipheriv, crypto.createDecipheriv]
    ) {
      // deno-lint-ignore no-explicit-any
      const stream = (factory as any)(algorithm, key, iv);
      const err = assertThrows(
        // deno-lint-ignore no-explicit-any
        () => stream.update(value as any),
        TypeError,
        'The "data" argument must be of type string or an instance of ' +
          "Buffer, TypedArray, or DataView",
      ) as Error & { code?: string };
      assertEquals(err.code, "ERR_INVALID_ARG_TYPE");
      try {
        stream.final();
      } catch { /* release native resource */ }
    }
  }
}

Deno.test({
  name:
    "Cipheriv/Decipheriv update() throws ERR_INVALID_ARG_TYPE for invalid data type",
  fn() {
    // Node.js uses `ArrayBuffer.isView(data)`, which rejects raw
    // ArrayBuffer / SharedArrayBuffer (only TypedArray / DataView pass).
    const invalidValues: readonly unknown[] = [
      123,
      true,
      null,
      undefined,
      {},
      [],
      Symbol("nope"),
      0n,
      new ArrayBuffer(16),
      new SharedArrayBuffer(16),
    ];

    // Cover several block / stream / AEAD modes to ensure the validation
    // applies regardless of the underlying cipher type.
    assertUpdateRejects("aes-256-cbc", 32, 16, invalidValues);
    assertUpdateRejects("aes-128-cbc", 16, 16, invalidValues);
    assertUpdateRejects("aes-128-ctr", 16, 16, invalidValues);
    assertUpdateRejects("aes-256-gcm", 32, 12, invalidValues);
  },
});

Deno.test({
  name:
    "Cipheriv/Decipheriv update() accepts string, Buffer, and TypedArray without throwing",
  fn() {
    const key = Buffer.alloc(32);
    const iv = Buffer.alloc(16);
    // Buffer is the typed surface for binary input; Uint8Array/Uint16Array
    // round-trip through Buffer.from to satisfy the typed `update()`
    // overload, while still exercising the runtime validation against
    // distinct backing storage shapes.
    const validInputs: readonly (string | Buffer)[] = [
      "hello world",
      Buffer.from("hello world"),
      Buffer.from(new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8])),
      Buffer.from(new Uint16Array([1, 2, 3, 4]).buffer),
    ];

    for (const input of validInputs) {
      const cipher = crypto.createCipheriv("aes-256-cbc", key, iv);
      // Round-trip to confirm the validation does not break the happy path.
      const head = typeof input === "string"
        ? cipher.update(input, "utf8")
        : cipher.update(input);
      const enc = Buffer.concat([head, cipher.final()]);
      assert(enc.length > 0);

      const decipher = crypto.createDecipheriv("aes-256-cbc", key, iv);
      const dec = Buffer.concat([decipher.update(enc), decipher.final()]);
      assert(dec.length > 0);
    }
  },
});

Deno.test({
  name:
    "Cipheriv/Decipheriv update() leaves stream usable after type-error throw",
  fn() {
    const key = Buffer.alloc(32);
    const iv = Buffer.alloc(16);

    // After update() rejects bad input, a subsequent valid update() + final()
    // should still produce ciphertext that round-trips correctly. This guards
    // against the validation accidentally finalizing the cipher state.
    const cipher = crypto.createCipheriv("aes-256-cbc", key, iv);
    assertThrows(
      // deno-lint-ignore no-explicit-any
      () => cipher.update(42 as any),
      TypeError,
    );
    const enc = Buffer.concat([cipher.update("ok", "utf8"), cipher.final()]);

    const decipher = crypto.createDecipheriv("aes-256-cbc", key, iv);
    assertThrows(
      // deno-lint-ignore no-explicit-any
      () => decipher.update(42 as any),
      TypeError,
    );
    const dec = Buffer.concat([decipher.update(enc), decipher.final()]);
    assertEquals(dec.toString("utf8"), "ok");
  },
});

Deno.test({
  name:
    "Cipheriv/Decipheriv final(encoding) flushes StringDecoder for stream ciphers",
  fn() {
    // Regression test for https://github.com/denoland/deno/issues/35797:
    // stream-mode ciphers (GCM/CTR/ChaCha20) took early-return paths in
    // final() that returned "" without flushing the StringDecoder used by
    // update(). A base64 decoder withholds up to 2 trailing bytes until
    // end(), so any plaintext whose byte length was not a multiple of 3 was
    // silently truncated with no error.
    const key = Buffer.alloc(32, 1);
    const plaintext = "hello"; // 5 bytes, 5 % 3 !== 0

    for (
      const [algo, iv, isAead] of [
        ["aes-256-gcm", Buffer.alloc(12, 2), true],
        ["aes-256-ctr", Buffer.alloc(16, 2), false],
        ["chacha20-poly1305", Buffer.alloc(12, 2), true],
      ] as const
    ) {
      const cipher = crypto.createCipheriv(algo, key, iv);
      const enc = cipher.update(plaintext, "utf8", "base64") +
        cipher.final("base64");
      // The full ciphertext must survive base64 round-tripping.
      assertEquals(
        Buffer.from(enc, "base64").length,
        plaintext.length,
        `${algo}: encrypted length`,
      );

      const decipher = crypto.createDecipheriv(algo, key, iv);
      if (isAead) {
        (decipher as crypto.DecipherGCM).setAuthTag(
          (cipher as crypto.CipherGCM).getAuthTag(),
        );
      }
      const dec = decipher.update(enc, "base64", "utf8") +
        decipher.final("utf8");
      assertEquals(dec, plaintext, `${algo}: round-trip`);
    }
  },
});

Deno.test({
  name: "Decipheriv final(utf8) flushes a truncated multibyte tail as U+FFFD",
  fn() {
    // Exercises the Decipheriv-side final() flush directly: the plaintext
    // ends with a lone UTF-8 lead byte, so the utf8 StringDecoder buffers
    // that byte during update() and only end() (invoked by final()) emits
    // the U+FFFD replacement char. Before the fix, final() returned "" and
    // dropped it. Matches Node, which returns "a�".
    const key = Buffer.alloc(32, 1);
    const iv = Buffer.alloc(16, 2);
    const raw = Buffer.from([0x61, 0xc3]); // "a" + incomplete 2-byte lead

    const cipher = crypto.createCipheriv("aes-256-ctr", key, iv);
    const enc = Buffer.concat([cipher.update(raw), cipher.final()]);

    const decipher = crypto.createDecipheriv("aes-256-ctr", key, iv);
    const dec = decipher.update(enc, undefined, "utf8") +
      decipher.final("utf8");
    assertEquals(dec, "a�");
  },
});

Deno.test({
  name: "Cipheriv/Decipheriv AES key wrap flushes StringDecoder in final()",
  fn() {
    // The AES key-wrap path computes its whole output in update() and takes
    // an early return in final(). With a base64 output encoding the decoder
    // buffers the trailing bytes, so final() must flush them too.
    const kek = Buffer.alloc(32, 7);
    // 24-byte key -> 32-byte wrapped output -> 32 % 3 === 2 buffered bytes.
    const keyToWrap = Buffer.alloc(24, 9);
    const iv = Buffer.alloc(8, 0xa6);

    const cipher = crypto.createCipheriv("aes256-wrap", kek, iv);
    const wrapped = cipher.update(keyToWrap, undefined, "base64") +
      cipher.final("base64");
    assertEquals(
      Buffer.from(wrapped, "base64").length,
      keyToWrap.length + 8,
      "wrapped length",
    );

    const decipher = crypto.createDecipheriv("aes256-wrap", kek, iv);
    const unwrapped = Buffer.concat([
      decipher.update(wrapped, "base64"),
      decipher.final(),
    ]);
    assertEquals(unwrapped, keyToWrap, "unwrap round-trip");

    // final() with an unknown output encoding now throws ERR_UNKNOWN_ENCODING
    // on the wrap path instead of silently returning "" (matches Node).
    const bad = crypto.createCipheriv("aes256-wrap", kek, iv);
    bad.update(keyToWrap);
    assertThrows(
      // deno-lint-ignore no-explicit-any
      () => bad.final("not-an-encoding" as any),
      Error,
      "Unknown encoding",
    );
  },
});
