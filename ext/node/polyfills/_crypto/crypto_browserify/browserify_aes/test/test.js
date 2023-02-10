// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2014-2017 browserify-aes contributors. All rights reserved. MIT license.
// Copyright 2013 Maxwell Krohn. All rights reserved. MIT license.
// Copyright 2009-2013 Jeff Mott. All rights reserved. MIT license.

import { Buffer } from "../../../../buffer.ts";
import {
  assert,
  assertEquals,
  assertThrows,
} from "../../../../../testing/asserts.ts";
import { fromFileUrl } from "../../../../path.ts";
import * as crypto from "../mod.js";
import { MODES } from "../modes/mod.js";
const CIPHERS = Object.keys(MODES);

const fixtures = JSON.parse(
  await Deno.readTextFile(
    fromFileUrl(new URL("./fixtures.json", import.meta.url)),
  ),
);
const fixtures2 = JSON.parse(
  await Deno.readTextFile(
    fromFileUrl(new URL("./extra.json", import.meta.url)),
  ),
);

function isGCM(cipher) {
  return MODES[cipher].mode === "GCM";
}

fixtures.forEach(function (f, i) {
  CIPHERS.forEach(function (cipher) {
    if (isGCM(cipher)) return;

    Deno.test("fixture " + i + " " + cipher, async function () {
      const suite = crypto.createCipher(cipher, Buffer.from(f.password));
      let buf = Buffer.alloc(0);
      suite.on("data", function (d) {
        buf = Buffer.concat([buf, d]);
      });
      suite.on("error", function (e) {
        console.log(e);
      });
      const p = new Promise((resolve, reject) => {
        suite.on("end", function () {
          try {
            assertEquals(buf.toString("hex"), f.results.ciphers[cipher]);
            resolve();
          } catch (e) {
            reject(e);
          }
        });
      });
      suite.write(Buffer.from(f.text));
      suite.end();
      await p;
    });

    Deno.test("fixture " + i + " " + cipher + "-decrypt", async function () {
      const suite = crypto.createDecipher(cipher, Buffer.from(f.password));
      let buf = Buffer.alloc(0);
      suite.on("data", function (d) {
        buf = Buffer.concat([buf, d]);
      });
      suite.on("error", function (e) {
        console.log(e);
      });
      const p = new Promise((resolve, reject) => {
        suite.on("end", function () {
          try {
            assertEquals(buf.toString("utf8"), f.text);
            resolve();
          } catch (e) {
            reject(e);
          }
        });
      });
      suite.write(Buffer.from(f.results.ciphers[cipher], "hex"));
      suite.end();
      await p;
    });
  });
});

fixtures2.forEach((f, i) => {
  Deno.test("test case " + i, function () {
    if (CIPHERS.indexOf(f.algo) === -1) {
      console.log("skipping unsupported " + f.algo + " test");
      return;
    }

    (function () {
      const encrypt = crypto.createCipheriv(
        f.algo,
        Buffer.from(f.key, "hex"),
        Buffer.from(f.iv, "hex"),
      );
      if (f.aad) encrypt.setAAD(Buffer.from(f.aad, "hex"));

      let hex = encrypt.update(f.plain, "utf-8", "hex");
      hex += encrypt.final("hex");
      const authTag = encrypt.getAuthTag();

      // only test basic encryption run if output is marked as tampered.
      if (!f.tampered) {
        assertEquals(hex.toUpperCase(), f.ct);
        assertEquals(authTag.toString("hex").toUpperCase(), f.tag);
      }
    })();
    (function () {
      const decrypt = crypto.createDecipheriv(
        f.algo,
        Buffer.from(f.key, "hex"),
        Buffer.from(f.iv, "hex"),
      );
      decrypt.setAuthTag(Buffer.from(f.tag, "hex"));
      if (f.aad) decrypt.setAAD(Buffer.from(f.aad, "hex"));
      let msg = decrypt.update(f.ct, "hex", "utf-8");
      if (!f.tampered) {
        msg += decrypt.final("utf-8");
        assertEquals(msg, f.plain);
      } else {
        // assert that final throws if input data could not be verified!
        assertThrows(
          function () {
            decrypt.final("utf-8");
          },
          undefined,
          " auth",
        );
      }
    })();
    (function () {
      if (!f.password) return;
      const encrypt = crypto.createCipher(f.algo, f.password);
      if (f.aad) encrypt.setAAD(Buffer.from(f.aad, "hex"));
      let hex = encrypt.update(f.plain, "utf-8", "hex");
      hex += encrypt.final("hex");
      const authTag = encrypt.getAuthTag();
      // only test basic encryption run if output is marked as tampered.
      if (!f.tampered) {
        assertEquals(hex.toUpperCase(), f.ct);
        assertEquals(authTag.toString("hex").toUpperCase(), f.tag);
      }
    })();
    (function () {
      if (!f.password) return;
      const decrypt = crypto.createDecipher(f.algo, f.password);
      decrypt.setAuthTag(Buffer.from(f.tag, "hex"));
      if (f.aad) decrypt.setAAD(Buffer.from(f.aad, "hex"));
      let msg = decrypt.update(f.ct, "hex", "utf-8");
      if (!f.tampered) {
        msg += decrypt.final("utf-8");
        assertEquals(msg, f.plain);
      } else {
        // assert that final throws if input data could not be verified!
        assertThrows(
          function () {
            decrypt.final("utf-8");
          },
          undefined,
          " auth",
        );
      }
    })();

    // after normal operation, test some incorrect ways of calling the API:
    // it's most certainly enough to run these tests with one algorithm only.
    if (i !== 0) {
      return;
    }

    (function () {
      // non-authenticating mode:
      const encrypt = crypto.createCipheriv(
        "aes-128-cbc",
        "ipxp9a6i1Mb4USb4",
        "6fKjEjR3Vl30EUYC",
      );
      encrypt.update("blah", "utf-8");
      encrypt.final();
      assertThrows(function () {
        encrypt.getAuthTag();
      });
      assertThrows(function () {
        encrypt.setAAD(Buffer.from("123", "utf-8"));
      });
    })();
    (function () {
      // trying to get tag before inputting all data:
      const encrypt = crypto.createCipheriv(
        f.algo,
        Buffer.from(f.key, "hex"),
        Buffer.from(f.iv, "hex"),
      );
      encrypt.update("blah", "utf-8");
      assertThrows(
        function () {
          encrypt.getAuthTag();
        },
        undefined,
        " state",
      );
    })();
    (function () {
      // trying to set tag on encryption object:
      const encrypt = crypto.createCipheriv(
        f.algo,
        Buffer.from(f.key, "hex"),
        Buffer.from(f.iv, "hex"),
      );
      assertThrows(
        function () {
          encrypt.setAuthTag(Buffer.from(f.tag, "hex"));
        },
        undefined,
        " state",
      );
    })();
    (function () {
      // trying to read tag from decryption object:
      const decrypt = crypto.createDecipheriv(
        f.algo,
        Buffer.from(f.key, "hex"),
        Buffer.from(f.iv, "hex"),
      );
      assertThrows(
        function () {
          decrypt.getAuthTag();
        },
        undefined,
        " state",
      );
    })();
  });
});

Deno.test("getCiphers works", function () {
  assert(crypto.getCiphers().length, "get some ciphers");
});

const gcmTest = [
  {
    key: "68d010dad5295e1f4f485f35cff46c35d423797bf4cd536d4943d787e00f6f07",
    length: 8,
    answer: "44d0f292",
    tag: "1f21c63664fc5262827b9624dee894bd",
    ivFill: 9,
  },
  {
    key: "9ba693ec61afc9b7950f9177780b3533126af40a7596c662e26e6d6bbf536030",
    length: 16,
    answer: "1c8f8783",
    tag: "2d2b33f509153a8afc973cf9fc983800",
    ivFill: 1,
  },
  {
    key: "dad2a11c52614e4402f0f126028d5e55b50b3a9d6d006cfbee79b77e4a4ee7b9",
    length: 21,
    ivFill: 2,
    answer: "1a8dd3ed",
    tag: "68ce0e40ee335388c0468813b8e5eb4b",
  },
  {
    key: "4c062c7bd7566bec4c509e3bf0c9cc2acb75a863403b04fdce025ba26b6a6ca2",
    length: 43,
    ivFill: 5,
    answer: "5f6ccc8c",
    tag: "9a0d845168a1491e17217a20a75defb0",
  },
];
function testIV(t, length, answer, tag, key, ivFill) {
  return t.step("key length " + length, function () {
    const iv = Buffer.alloc(length, ivFill);
    const cipher = crypto.createCipheriv("aes-256-gcm", key, iv);
    const out = cipher.update("fooo").toString("hex");
    assertEquals(out, answer);
    cipher.final();
    assertEquals(tag, cipher.getAuthTag().toString("hex"));
    const decipher = crypto.createDecipheriv("aes-256-gcm", key, iv);
    decipher.setAuthTag(Buffer.from(tag, "hex"));
    const decrypted = decipher.update(Buffer.from(answer, "hex"));
    assertEquals(decrypted.toString(), "fooo");
  });
}
Deno.test("different IV lengths work for GCM", async function (t) {
  for (const item of gcmTest) {
    await testIV(
      t,
      item.length,
      item.answer,
      item.tag,
      Buffer.from(item.key, "hex"),
      item.ivFill,
    );
  }
});

Deno.test("handle long uft8 plaintexts", function () {
  const salt = Buffer.alloc(32, 0);

  function encrypt(txt) {
    const cipher = crypto.createCipher("aes-256-cbc", salt);
    return cipher.update(txt, "utf8", "base64") + cipher.final("base64");
  }

  function decrypt(enc) {
    const decipher = crypto.createDecipher("aes-256-cbc", salt);
    return decipher.update(enc, "base64", "utf8") + decipher.final("utf8");
  }

  const input =
    "ふっかつ　あきる　すぶり　はやい　つける　まゆげ　たんさん　みんぞく　ねほりはほり　せまい　たいまつばな　ひはん";
  const enc = encrypt(input, "a");

  const dec = decrypt(enc, "a");
  assertEquals(dec, input);
});

Deno.test("mix and match encoding", function () {
  let cipher = crypto.createCipher("aes-256-cbc", "a");
  cipher.update("foo", "utf8", "utf8");
  assertThrows(function () {
    cipher.update("foo", "utf8", "base64");
  });
  cipher = crypto.createCipher("aes-256-cbc", "a");
  cipher.update("foo", "utf8", "base64");
  cipher.update("foo", "utf8");
  cipher.final("base64");
});
