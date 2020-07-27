// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { HmacSha256, Message, Sha256 } from "./sha256.ts";
import { assertEquals } from "../testing/asserts.ts";
import { fromFileUrl, join, parent, resolve } from "../path/mod.ts";

const moduleDir = parent(fromFileUrl(import.meta.url));
const testdataDir = resolve(moduleDir, "testdata");

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
  sha256: Record<string, Record<string, Message>>;
  sha224: Record<string, Record<string, Message>>;
  sha256Hmac: Record<string, Record<string, [Message, Message]>>;
  sha224Hmac: Record<string, Record<string, [Message, Message]>>;
} = {
  sha256: {
    "ascii": {
      "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855": "",
      "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592": "The quick brown fox jumps over the lazy dog",
      "ef537f25c895bfa782526529a9b63d97aa631564d5d789c2b765448c8635fb6c": "The quick brown fox jumps over the lazy dog."
    },
    "ascii more than 64 bytes": {
      "54e73d89e1924fdcd056390266a983924b6d6d461e9470b6cd50bbaf69b5c54c": "The MD5 message-digest algorithm is a widely used cryptographic hash function producing a 128-bit (16-byte) hash value, typically expressed in text format as a 32 digit hexadecimal number. MD5 has been utilized in a wide variety of cryptographic applications, and is also commonly used to verify data integrity."
    },
    "UTF8": {
      "72726d8818f693066ceb69afa364218b692e62ea92b385782363780f47529c21": "中文",
      "53196d1acfce0c4b264e01e8018c989d571351f59e33f055f76ff15b4f0516c6": "aécio",
      "8d10a48685dbc34484696de7ea7434d80a54c1d60100530faccf697463ef19c9": "𠜎"
    },
    "UTF8 more than 64 bytes": {
      "d691014feebf35b3500ef6f6738d0094cac63628a7a018a980a40292a77703d1": "訊息摘要演算法第五版（英語：Message-Digest Algorithm 5，縮寫為MD5），是當前電腦領域用於確保資訊傳輸完整一致而廣泛使用的雜湊演算法之一",
      "81a1472ebdeb09406a783d607ff49ee2fde3e9f44ac1cd158ad8d6ad3c4e69fa": "訊息摘要演算法第五版（英語：Message-Digest Algorithm 5，縮寫為MD5），是當前電腦領域用於確保資訊傳輸完整一致而廣泛使用的雜湊演算法之一（又譯雜湊演算法、摘要演算法等），主流程式語言普遍已有MD5的實作。"
    },
    "special length": {
      "5e6b963e2b6444dab8544beab8532850cef2a9d143872a6a5384abe37e61b3db": "0123456780123456780123456780123456780123456780123456780",
      "85d240a4a03a0710423fc4f701da51e8785c9eaa96d718ab1c7991d6afd60d62": "01234567801234567801234567801234567801234567801234567801",
      "c3ee464d5620eb2dde3dfda4c7955dbd9e9e2e9b113c13983fc67b0dfd892a53": "0123456780123456780123456780123456780123456780123456780123456780",
      "74b51c6911f9a8b5e7c499effe7604e43b672166818873c27752c248de434841": "01234567801234567801234567801234567801234567801234567801234567801234567",
      "6fba9e623ae6abf028a1b195748814aa95eebfb22e3ec5e15d2444cd6c48186a": "012345678012345678012345678012345678012345678012345678012345678012345678"
    },
    "Array": {
      "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855": [],
      "182889f925ae4e5cc37118ded6ed87f7bdc7cab5ec5e78faef2e50048999473f": [211, 212],
      "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592": [84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103],
      "74b51c6911f9a8b5e7c499effe7604e43b672166818873c27752c248de434841": [48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55]
    }
  },
  sha224: {
    "ascii": {
      "d14a028c2a3a2bc9476102bb288234c415a2b01f828ea62ac5b3e42f": "",
      "730e109bd7a8a32b1cb9d9a09aa2325d2430587ddbc0c38bad911525": "The quick brown fox jumps over the lazy dog",
      "619cba8e8e05826e9b8c519c0a5c68f4fb653e8a3d8aa04bb2c8cd4c": "The quick brown fox jumps over the lazy dog."
    },
    "ascii more than 64 bytes": {
      "4d97e15967391d2e846ea7d21bb480efadbae5868b731e7cc6267006": "The MD5 message-digest algorithm is a widely used cryptographic hash function producing a 128-bit (16-byte) hash value, typically expressed in text format as a 32 digit hexadecimal number. MD5 has been utilized in a wide variety of cryptographic applications, and is also commonly used to verify data integrity."
    },
    "UTF8": {
      "dfbab71afdf54388af4d55f8bd3de8c9b15e0eb916bf9125f4a959d4": "中文",
      "d12841cafd89c534924a839e62bf35a2b5f3717b7802eb19bd8d8e15": "aécio",
      "eaa0129b5509f5701db218fb7076b282e4409da52d06363aa3bdd63d": "𠜎"
    },
    "UTF8 more than 64 bytes": {
      "0dda421f3f81272418e1313673e9d74b7f2d04efc9c52c69458e12c3": "訊息摘要演算法第五版（英語：Message-Digest Algorithm 5，縮寫為MD5），是當前電腦領域用於確保資訊傳輸完整一致而廣泛使用的雜湊演算法之一",
      "a8cb74a54e6dc6ab6110db3915ba08ffe5e1abafaea78538fa12a626": "訊息摘要演算法第五版（英語：Message-Digest Algorithm 5，縮寫為MD5），是當前電腦領域用於確保資訊傳輸完整一致而廣泛使用的雜湊演算法之一（又譯雜湊演算法、摘要演算法等），主流程式語言普遍已有MD5的實作。"
    },
    "special length": {
      "bc4a354d66f3cff4bc6dd6a88fbb0435cede7fd5fe94da0760cb1924": "0123456780123456780123456780123456780123456780123456780",
      "2f148f757d1295784a7c69bf328b8bf827a536669e132234cd6f50e7": "01234567801234567801234567801234567801234567801234567801",
      "496275a96bf41aa27ce89c3ae0fc63c3a3eab063887a8ea075bd091b": "0123456780123456780123456780123456780123456780123456780123456780",
      "16ee1b101fe0e0d8dd156d598931ec19d75b0f8dc0a0455733c168c8": "01234567801234567801234567801234567801234567801234567801234567801234567",
      "04c7a30079c640e440d884cdf0d7ab04fd05501d4498cb21be29ca1f": "012345678012345678012345678012345678012345678012345678012345678012345678"
    },
    "Array": {
      "d14a028c2a3a2bc9476102bb288234c415a2b01f828ea62ac5b3e42f": [],
      "730e109bd7a8a32b1cb9d9a09aa2325d2430587ddbc0c38bad911525": [84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103],
      "16ee1b101fe0e0d8dd156d598931ec19d75b0f8dc0a0455733c168c8": [48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55]
    }
  },
  sha256Hmac: {
    "Test Vectors": {
      "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7": [
        [0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b],
        "Hi There"
      ],
      "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843": [
        "Jefe",
        "what do ya want for nothing?"
      ],
      "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        [0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd]
      ],
      "82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b": [
        [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19],
        [0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd]
      ],
      "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        "Test Using Larger Than Block-Size Key - Hash Key First"
      ],
      "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        "This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm."
      ]
    },
    "UTF8": {
      "865cc329d317f6d9fdbd183a3c5cc5fd4c370d11f98abbbb404bceb1e6392c7e": ["中文", "中文"],
      "efeef87be5731506b69bb64a9898a456dd12c94834c36a4d8ba99e3db79ad7ed": ["aécio", "aécio"],
      "8a6e527049b9cfc7e1c84bcf356a1289c95da68a586c03de3327f3de0d3737fe": ["𠜎", "𠜎"]
    }
  },
  sha224Hmac: {
    "Test Vectors": {
      "896fb1128abbdf196832107cd49df33f47b4b1169912ba4f53684b22": [
        [0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b],
        "Hi There"
      ],
      "a30e01098bc6dbbf45690f3a7e9e6d0f8bbea2a39e6148008fd05e44": [
        "Jefe",
        "what do ya want for nothing?"
      ],
      "7fb3cb3588c6c1f6ffa9694d7d6ad2649365b0c1f65d69d1ec8333ea": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        [0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd]
      ],
      "6c11506874013cac6a2abc1bb382627cec6a90d86efc012de7afec5a": [
        [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19],
        [0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd]
      ],
      "95e9a0db962095adaebe9b2d6f0dbce2d499f112f2d2b7273fa6870e": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        "Test Using Larger Than Block-Size Key - Hash Key First"
      ],
      "3a854166ac5d9f023f54d517d0b39dbd946770db9c2b95c9f6f565d1": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        "This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm."
      ]
    },
    "UTF8": {
      "e2280928fe813aeb7fa59aa14dd5e589041bfdf91945d19d25b9f3db": ["中文", "中文"],
      "86c53dc054b16f6e006a254891bc9ff0da5df8e1a6faee3b0aaa732d": ["aécio", "aécio"],
      "e9e5991bfb84506b105f800afac1599ff807bb8e20db8ffda48997b9": ["𠜎", "𠜎"]
    }
  },
};

// deno-fmt-ignore
fixtures.sha256.Uint8Array = {
  '182889f925ae4e5cc37118ded6ed87f7bdc7cab5ec5e78faef2e50048999473f': new Uint8Array([211, 212]),
  'd7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592': new Uint8Array([84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103])
};
// deno-fmt-ignore
fixtures.sha256.Int8Array = {
  'd7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592': new Int8Array([84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103])
};
// deno-fmt-ignore
fixtures.sha256.ArrayBuffer = {
  'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855': new ArrayBuffer(0),
  '6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d': new ArrayBuffer(1)
};
// deno-fmt-ignore
fixtures.sha224.Uint8Array = {
  'e17541396a3ecd1cd5a2b968b84e597e8eae3b0ea3127963bf48dd3b': new Uint8Array([211, 212]),
  '730e109bd7a8a32b1cb9d9a09aa2325d2430587ddbc0c38bad911525': new Uint8Array([84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103])
};
// deno-fmt-ignore
fixtures.sha224.Int8Array = {
  '730e109bd7a8a32b1cb9d9a09aa2325d2430587ddbc0c38bad911525': new Int8Array([84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103])
};
// deno-fmt-ignore
fixtures.sha224.ArrayBuffer = {
  'd14a028c2a3a2bc9476102bb288234c415a2b01f828ea62ac5b3e42f': new ArrayBuffer(0),
  'fff9292b4201617bdc4d3053fce02734166a683d7d858a7f5f59b073': new ArrayBuffer(1),
};
fixtures.sha256Hmac.Uint8Array = {
  e48411262715c8370cd5e7bf8e82bef53bd53712d007f3429351843b77c7bb9b: [
    new Uint8Array(0),
    "Hi There",
  ],
};
fixtures.sha256Hmac.ArrayBuffer = {
  e48411262715c8370cd5e7bf8e82bef53bd53712d007f3429351843b77c7bb9b: [
    new ArrayBuffer(0),
    "Hi There",
  ],
};
fixtures.sha224Hmac.Uint8Array = {
  da8f94de91d62154b55ea4e8d6eb133f6d553bcd1f1ba205b9488945: [
    new Uint8Array(0),
    "Hi There",
  ],
};
fixtures.sha224Hmac.ArrayBuffer = {
  da8f94de91d62154b55ea4e8d6eb133f6d553bcd1f1ba205b9488945: [
    new ArrayBuffer(0),
    "Hi There",
  ],
};

const methods = ["array", "arrayBuffer", "digest", "hex"] as const;

for (const method of methods) {
  for (const [name, tests] of Object.entries(fixtures.sha256)) {
    let i = 1;
    for (const [expected, message] of Object.entries(tests)) {
      Deno.test({
        name: `sha256.${method}() - ${name} - #${i++}`,
        fn() {
          const algorithm = new Sha256();
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
  for (const [name, tests] of Object.entries(fixtures.sha224)) {
    let i = 1;
    for (const [expected, message] of Object.entries(tests)) {
      Deno.test({
        name: `sha224.${method}() - ${name} - #${i++}`,
        fn() {
          const algorithm = new Sha256(true);
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
  for (const [name, tests] of Object.entries(fixtures.sha256Hmac)) {
    let i = 1;
    for (const [expected, [key, message]] of Object.entries(tests)) {
      Deno.test({
        name: `hmacSha256.${method}() - ${name} - #${i++}`,
        fn() {
          const algorithm = new HmacSha256(key);
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
  for (const [name, tests] of Object.entries(fixtures.sha224Hmac)) {
    let i = 1;
    for (const [expected, [key, message]] of Object.entries(tests)) {
      Deno.test({
        name: `hmacSha224.${method}() - ${name} - #${i++}`,
        fn() {
          const algorithm = new HmacSha256(key, true);
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

Deno.test("[hash/sha256] test Uint8Array from Reader", async () => {
  const data = await Deno.readFile(join(testdataDir, "hashtest"));

  const hash = new Sha256().update(data).hex();
  assertEquals(
    hash,
    "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
  );
});
