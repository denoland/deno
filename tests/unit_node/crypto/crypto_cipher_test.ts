// Copyright 2018-2025 the Deno authors. MIT license.
import crypto from "node:crypto";
import { Buffer } from "node:buffer";
import { Readable } from "node:stream";
import { buffer, text } from "node:stream/consumers";
import { assert, assertEquals, assertThrows } from "@std/assert";

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
  name: "rsa private encrypt and private decrypt",
  fn() {
    const encrypted = crypto.privateEncrypt(rsaPrivateKey, input);
    assert(Buffer.isBuffer(encrypted));
    const decrypted = crypto.privateDecrypt(
      rsaPrivateKey,
      Buffer.from(encrypted),
    );
    assert(Buffer.isBuffer(decrypted));
    assertEquals(decrypted, input);
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
      TypeError,
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
      ["aes256", 31, 16, Invalid.Key],
      ["aes-256-cbc", 31, 16, Invalid.Key],
      ["aes256", 32, 15, Invalid.Iv],
      ["aes-256-cbc", 32, 15, Invalid.Iv],
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
      TypeError,
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
      ["aes256", 31, 16, Invalid.Key],
      ["aes-256-cbc", 31, 16, Invalid.Key],
      ["aes256", 32, 15, Invalid.Iv],
      ["aes-256-cbc", 32, 15, Invalid.Iv],
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

    const getZeroKey = (cipher: string) => zeros(+cipher.match(/\d+/)![0] / 8);
    const getZeroIv = (cipher: string) => {
      if (cipher.includes("gcm") || cipher.includes("ecb")) {
        return zeros(12);
      }
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
      "Wrong final block length",
    );
  },
});
