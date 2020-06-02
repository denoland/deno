// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { createHash } from "./mod.ts";

const millionAs = "a".repeat(1000000);

const testSetHex = [
  [
    "",
    "0c63a75b845e4f7d01107d852e4c2485c51a50aaaa94fc61995e71bbee983a2ac3713831264adb47fb6bd1e058d5f004",
  ],
  [
    "abc",
    "ec01498288516fc926459f58e2c6ad8df9b473cb0fc08c2596da7cf0e49be4b298d88cea927ac7f539f1edf228376d25",
  ],
  [
    "deno",
    "9cb19574077f07a44d980e9e84bc155951f37d97fa527ae6007cb0252274d8b392523110d10101cef1f0bde11fd95dee",
  ],
  [
    "The quick brown fox jumps over the lazy dog",
    "7063465e08a93bce31cd89d2e3ca8f602498696e253592ed26f07bf7e703cf328581e1471a7ba7ab119b1a9ebdf8be41",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "2777b6ee7309657e7feb159e001af7f5a69a24fe6aedab05ef575cb260b5ca9d4dee4fc9a68dec0e6f820b88a6369a04",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "2d811f3045bb42b43b8ecadd41ccc1391be8ad805ac626ed4ecbaa6c538032b832437baf3b89e8e56e83f47e9183045d",
  ],
  [
    millionAs,
    "eee9e24d78c1855337983451df97c8ad9eedf256c6334f8e948d252d5e0e76847aa0774ddb90a842190d2c558b4b8340",
  ],
];

const testSetBase64 = [
  ["", "DGOnW4ReT30BEH2FLkwkhcUaUKqqlPxhmV5xu+6YOirDcTgxJkrbR/tr0eBY1fAE"],
  ["abc", "7AFJgohRb8kmRZ9Y4satjfm0c8sPwIwlltp88OSb5LKY2IzqknrH9Tnx7fIoN20l"],
  ["deno", "nLGVdAd/B6RNmA6ehLwVWVHzfZf6UnrmAHywJSJ02LOSUjEQ0QEBzvHwveEf2V3u"],
  [
    "The quick brown fox jumps over the lazy dog",
    "cGNGXgipO84xzYnS48qPYCSYaW4lNZLtJvB79+cDzzKFgeFHGnunqxGbGp69+L5B",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "J3e27nMJZX5/6xWeABr39aaaJP5q7asF71dcsmC1yp1N7k/Jpo3sDm+CC4imNpoE",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "LYEfMEW7QrQ7jsrdQczBORvorYBaxibtTsuqbFOAMrgyQ3uvO4no5W6D9H6RgwRd",
  ],
  [
    millionAs,
    "7uniTXjBhVM3mDRR35fIrZ7t8lbGM0+OlI0lLV4OdoR6oHdN25CoQhkNLFWLS4NA",
  ],
];

test("[hash/sha3-384] testSha3-384Hex", () => {
  for (const [input, output] of testSetHex) {
    const sha3 = createHash("sha3-384");
    assertEquals(sha3.update(input).toString(), output);
    sha3.dispose();
  }
});

test("[hash/sha3-384] testSha3-384Base64", () => {
  for (const [input, output] of testSetBase64) {
    const sha3 = createHash("sha3-384");
    assertEquals(sha3.update(input).toString("base64"), output);
    sha3.dispose();
  }
});
