// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { createHash } from "./mod.ts";

const millionAs = "a".repeat(1000000);

const testSetHex = [
  [
    "",
    "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
  ],
  [
    "abc",
    "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
  ],
  [
    "deno",
    "05b6ef7b13673c57c455d968cb7acbdd4fe10e24f25520763d69025d768d14124987b14e3aff8ff1565aaeba1c405fc89cc435938ff46a426f697b0f509e3799",
  ],
  [
    "The quick brown fox jumps over the lazy dog",
    "07e547d9586f6a73f73fbac0435ed76951218fb7d0c8d788a309d785436bbb642e93a252a954f23912547d1e8a3b5ed6e1bfd7097821233fa0538f3db854fee6",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "962b64aae357d2a4fee3ded8b539bdc9d325081822b0bfc55583133aab44f18bafe11d72a7ae16c79ce2ba620ae2242d5144809161945f1367f41b3972e26e04",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "01d35c10c6c38c2dcf48f7eebb3235fb5ad74a65ec4cd016e2354c637a8fb49b695ef3c1d6f7ae4cd74d78cc9c9bcac9d4f23a73019998a7f73038a5c9b2dbde",
  ],
  [
    millionAs,
    "e718483d0ce769644e2e42c7bc15b4638e1f98b13b2044285632a803afa973ebde0ff244877ea60a4cb0432ce577c31beb009c5c2c49aa2e4eadb217ad8cc09b",
  ],
];

const testSetBase64 = [
  [
    "",
    "z4PhNX7vuL3xVChQ1m2AB9Yg5AULVxXcg/SpIdNs6c5H0NE8XYXysP+DGNKHfuwvY7kxvUdBeoGlODJ6+SfaPg==",
  ],
  [
    "abc",
    "3a81oZNherrMQXNJriBBMRLm+k6JqX6iCp7u5ktV05ohkpkqJ0/BqDa6PCOj/uu9RU1EI2Q86A4qmslPpUyknw==",
  ],
  [
    "deno",
    "BbbvexNnPFfEVdloy3rL3U/hDiTyVSB2PWkCXXaNFBJJh7FOOv+P8VZarrocQF/InMQ1k4/0akJvaXsPUJ43mQ==",
  ],
  [
    "The quick brown fox jumps over the lazy dog",
    "B+VH2VhvanP3P7rAQ17XaVEhj7fQyNeIownXhUNru2Quk6JSqVTyORJUfR6KO17W4b/XCXghIz+gU489uFT+5g==",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "litkquNX0qT+497YtTm9ydMlCBgisL/FVYMTOqtE8Yuv4R1yp64Wx5ziumIK4iQtUUSAkWGUXxNn9Bs5cuJuBA==",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "AdNcEMbDjC3PSPfuuzI1+1rXSmXsTNAW4jVMY3qPtJtpXvPB1veuTNdNeMycm8rJ1PI6cwGZmKf3MDilybLb3g==",
  ],
  [
    millionAs,
    "5xhIPQznaWROLkLHvBW0Y44fmLE7IEQoVjKoA6+pc+veD/JEh36mCkywQyzld8Mb6wCcXCxJqi5OrbIXrYzAmw==",
  ],
];
test("[hash/sha512] testSha512Hex", () => {
  for (const [input, output] of testSetHex) {
    const sha512 = createHash("sha512");
    assertEquals(sha512.update(input).toString(), output);
    sha512.dispose();
  }
});

test("[hash/sha512] testSha512Base64", () => {
  for (const [input, output] of testSetBase64) {
    const sha512 = createHash("sha512");
    assertEquals(sha512.update(input).toString("base64"), output);
    sha512.dispose();
  }
});
