// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import crypto from "node:crypto";
import { Buffer } from "node:buffer";
import { Readable } from "node:stream";
import { buffer, text } from "node:stream/consumers";
import { assertEquals, assertThrows } from "@std/assert";

const rsaPrivateKey = Deno.readTextFileSync(
  new URL("../testdata/rsa_private.pem", import.meta.url),
);
const rsaPublicKey = Deno.readTextFileSync(
  new URL("../testdata/rsa_public.pem", import.meta.url),
);

const input = new TextEncoder().encode("hello world");

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
    const decrypted = crypto.privateDecrypt(
      rsaPrivateKey,
      Buffer.from(encrypted),
    );
    assertEquals(decrypted, input);
  },
});

Deno.test({
  name: "rsa public decrypt fail",
  fn() {
    const encrypted = crypto.publicEncrypt(rsaPublicKey, input);
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
    ] as const;
    for (
      const [[alg, keyLen, ivLen], input] of table
    ) {
      const cipher = crypto.createDecipheriv(alg, zeros(keyLen), zeros(ivLen));
      assertEquals(cipher.update(input, "hex"), Buffer.alloc(80));
      assertEquals(cipher.final(), Buffer.alloc(10));
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
    assertThrows(
      () =>
        crypto.createCipheriv("aes256", new Uint8Array(31), new Uint8Array(16)),
      RangeError,
      "Invalid key length",
    );
    assertThrows(
      () =>
        crypto.createCipheriv(
          "aes-256-cbc",
          new Uint8Array(31),
          new Uint8Array(16),
        ),
      RangeError,
      "Invalid key length",
    );
    assertThrows(
      () =>
        crypto.createCipheriv("aes256", new Uint8Array(32), new Uint8Array(15)),
      TypeError,
      "Invalid initialization vector",
    );
    assertThrows(
      () =>
        crypto.createCipheriv(
          "aes-256-cbc",
          new Uint8Array(32),
          new Uint8Array(15),
        ),
      TypeError,
      "Invalid initialization vector",
    );
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
    assertThrows(
      () =>
        crypto.createDecipheriv(
          "aes256",
          new Uint8Array(31),
          new Uint8Array(16),
        ),
      RangeError,
      "Invalid key length",
    );
    assertThrows(
      () =>
        crypto.createDecipheriv(
          "aes-256-cbc",
          new Uint8Array(31),
          new Uint8Array(16),
        ),
      RangeError,
      "Invalid key length",
    );
    assertThrows(
      () =>
        crypto.createDecipheriv(
          "aes256",
          new Uint8Array(32),
          new Uint8Array(15),
        ),
      TypeError,
      "Invalid initialization vector",
    );
    assertThrows(
      () =>
        crypto.createDecipheriv(
          "aes-256-cbc",
          new Uint8Array(32),
          new Uint8Array(15),
        ),
      TypeError,
      "Invalid initialization vector",
    );
  },
});

Deno.test({
  name: "getCiphers",
  fn() {
    assertEquals(crypto.getCiphers().includes("aes-128-cbc"), true);
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
