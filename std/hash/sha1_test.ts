// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { Sha1, Message } from "./sha1.ts";
import { join, resolve } from "../path/mod.ts";

const testdataDir = resolve("hash", "testdata");

/** Handy function to convert an array/array buffer to a string of hex values. */
function toHexString(value: number[] | ArrayBuffer): string {
  const array = new Uint8Array(value);
  let hex = "";
  for (const v of array) {
    const c = v.toString(16);
    hex += c.length === 1 ? `0${c}` : c;
  }
  return hex;
}

// deno-fmt-ignore
const fixtures: {
  sha1: Record<string, Record<string, Message>>;
} = {
  sha1: {
    "ascii": {
      "da39a3ee5e6b4b0d3255bfef95601890afd80709": "",
      "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12": "The quick brown fox jumps over the lazy dog",
      "408d94384216f890ff7a0c3528e8bed1e0b01621": "The quick brown fox jumps over the lazy dog."
    },
    "ascii more than 64 bytes": {
      "8690faab7755408a03875895176fac318f14a699": "The MD5 message-digest algorithm is a widely used cryptographic hash function producing a 128-bit (16-byte) hash value, typically expressed in text format as a 32 digit hexadecimal number. MD5 has been utilized in a wide variety of cryptographic applications, and is also commonly used to verify data integrity."
    },
    "UTF8": {
      "7be2d2d20c106eee0836c9bc2b939890a78e8fb3": "中文",
      "9e4e5d978deced901d621475b03f1ded19e945bf": "aécio",
      "4667688a63420661469c8dbc0f871770349bab08": "𠜎"
    },
    "UTF8 more than 64 bytes": {
      "ad8aae581c915fe01c4964a5e8b322cae74ee5c5": "訊息摘要演算法第五版（英語：Message-Digest Algorithm 5，縮寫為MD5），是當前電腦領域用於確保資訊傳輸完整一致而廣泛使用的雜湊演算法之一",
      "3a15ad3ce9efdd4bf982eaaaecdeda36a887a3f9": "訊息摘要演算法第五版（英語：Message-Digest Algorithm 5，縮寫為MD5），是當前電腦領域用於確保資訊傳輸完整一致而廣泛使用的雜湊演算法之一（又譯雜湊演算法、摘要演算法等），主流程式語言普遍已有MD5的實作。"
    },
    "special length": {
      "4cdeae78e8b7285aef73e0a15eec7d5b30f3f3e3": "0123456780123456780123456780123456780123456780123456780",
      "e657e6bb6b5d0c2bf7e929451c14a5302589a60b": "01234567801234567801234567801234567801234567801234567801",
      "e7ad97591c1a99d54d80751d341899769884c75a": "0123456780123456780123456780123456780123456780123456780123456780",
      "55a13698cdc010c0d16dab2f7dc10f43a713f12f": "01234567801234567801234567801234567801234567801234567801234567801234567",
      "006575418c27b0158e55a6d261c46f86b33a496a": "012345678012345678012345678012345678012345678012345678012345678012345678"
    },
    "Array": {
      "da39a3ee5e6b4b0d3255bfef95601890afd80709": [],
      '2fd4e1c67a2d28fced849ee1bb76e7391b93eb12': [84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103],
      '55a13698cdc010c0d16dab2f7dc10f43a713f12f': [48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55]
    },
    "Uint8Array": {
      '2fd4e1c67a2d28fced849ee1bb76e7391b93eb12': new Uint8Array([84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103])
    },
    "Int8Array": {
      '2fd4e1c67a2d28fced849ee1bb76e7391b93eb12': new Int8Array([84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103])
    },
    "ArrayBuffer": {
      '5ba93c9db0cff93f52b521d7420e43f6eda2784f': new ArrayBuffer(1)
    }
  },
};

const methods = ["array", "arrayBuffer", "digest", "hex"] as const;

for (const method of methods) {
  for (const [name, tests] of Object.entries(fixtures.sha1)) {
    let i = 1;
    for (const [expected, message] of Object.entries(tests)) {
      Deno.test({
        name: `sha1.${method}() - ${name} - #${i++}`,
        fn() {
          const algorithm = new Sha1();
          algorithm.update(message);
          const actual = method === "hex"
            ? algorithm[method]()
            : toHexString(algorithm[method]());
          assertEquals(actual, expected);
        },
      });
    }
  }
}

for (const method of methods) {
  for (const [name, tests] of Object.entries(fixtures.sha1)) {
    let i = 1;
    for (const [expected, message] of Object.entries(tests)) {
      Deno.test({
        name: `sha1.${method}() - ${name} - #${i++}`,
        fn() {
          const algorithm = new Sha1(true);
          algorithm.update(message);
          const actual = method === "hex"
            ? algorithm[method]()
            : toHexString(algorithm[method]());
          assertEquals(actual, expected);
        },
      });
    }
  }
}

Deno.test("[hash/sha1] test Uint8Array from Reader", async () => {
  const data = await Deno.readFile(join(testdataDir, "hashtest"));

  const hash = new Sha1().update(data).hex();
  assertEquals(hash, "a94a8fe5ccb19ba61c4c0873d391e987982fbbd3");
});
