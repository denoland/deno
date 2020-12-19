// Test cases copied from https://github.com/LinusU/base32-encode/blob/master/test.js
// Copyright (c) 2016-2017 Linus Unnebäck. MIT license.
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "../testing/asserts.ts";
import { decode, encode } from "./base32.ts";

// Lifted from https://stackoverflow.com/questions/38987784
const fromHexString = (hexString: string): Uint8Array =>
  new Uint8Array(hexString.match(/.{1,2}/g)!.map((byte) => parseInt(byte, 16)));
const toHexString = (bytes: Uint8Array): string =>
  bytes.reduce((str, byte) => str + byte.toString(16).padStart(2, "0"), "");

const testCases = [
  ["73", "OM======"],
  ["f80c", "7AGA===="],
  ["6450", "MRIA===="],
  ["cc91d0", "ZSI5A==="],
  ["6c60c0", "NRQMA==="],
  ["4f6a23", "J5VCG==="],
  ["88b44f18", "RC2E6GA="],
  ["90bad04714", "SC5NARYU"],
  ["e9ef1def8086", "5HXR334AQY======"],
  ["83fe3f9c1e9302", "QP7D7HA6SMBA===="],
  ["15aa1f7cafc17cb8", "CWVB67FPYF6LQ==="],
  ["da51d4fed48b4c32dc", "3JI5J7WURNGDFXA="],
  ["c4be14228512d7299831", "YS7BIIUFCLLSTGBR"],
  ["2f273c5b5ef04724fab944", "F4TTYW266BDSJ6VZIQ======"],
  ["969da1b80ec2442d2bdd4bdb", "S2O2DOAOYJCC2K65JPNQ===="],
  ["31f5adb50792f549d3714f3f99", "GH223NIHSL2UTU3RJ47ZS==="],
  ["6a654f7a072c29951930700c0a61", "NJSU66QHFQUZKGJQOAGAUYI="],
  ["0fe29d6825ad999e87d9b7cac3589d", "B7RJ22BFVWMZ5B6ZW7FMGWE5"],
  ["0f960ab44e165973a5172ccd294b3412", "B6LAVNCOCZMXHJIXFTGSSSZUCI======"],
  ["325b9fd847a41fb0d485c207a1a5b02dcf", "GJNZ7WCHUQP3BVEFYID2DJNQFXHQ===="],
  ["ddf80ebe21bf1b1e12a64c5cc6a74b5d92dd", "3X4A5PRBX4NR4EVGJROMNJ2LLWJN2==="],
  [
    "c0cae52c6f641ce04a7ee5b9a8fa8ded121bca",
    "YDFOKLDPMQOOAST64W42R6UN5UJBXSQ=",
  ],
  [
    "872840a355c8c70586f462c9e669ee760cb3537e",
    "Q4UEBI2VZDDQLBXUMLE6M2POOYGLGU36",
  ],
  [
    "5773fe22662818a120c5688824c935fe018208a496",
    "K5Z74ITGFAMKCIGFNCECJSJV7YAYECFESY======",
  ],
  [
    "416e23abc524d1b85736e2bea6cfecd5192789034a28",
    "IFXCHK6FETI3QVZW4K7KNT7M2UMSPCIDJIUA====",
  ],
  [
    "83d2386ebdd7e8e818ec00e3ccd882aa933b905b7e2e44",
    "QPJDQ3V527UOQGHMADR4ZWECVKJTXEC3PYXEI===",
  ],
  [
    "a2fa8b881f3b8024f52745763c4ae08ea12bdf8bef1a72f8",
    "UL5IXCA7HOACJ5JHIV3DYSXAR2QSXX4L54NHF6A=",
  ],
  [
    "b074ae8b9efde0f17f37bccadde006d039997b59c8efb05add",
    "WB2K5C467XQPC7ZXXTFN3YAG2A4ZS62ZZDX3AWW5",
  ],
  [
    "764fef941aee7e416dc204ae5ab9c5b9ce644567798e6849aea9",
    "OZH67FA25Z7EC3OCASXFVOOFXHHGIRLHPGHGQSNOVE======",
  ],
  [
    "4995d9811f37f59797d7c3b9b9e5325aa78277415f70f4accf588c",
    "JGK5TAI7G72ZPF6XYO43TZJSLKTYE52BL5YPJLGPLCGA====",
  ],
  [
    "24f0812ca8eed58374c11a7008f0b262698b72fd2792709208eaacb2",
    "ETYICLFI53KYG5GBDJYAR4FSMJUYW4X5E6JHBEQI5KWLE===",
  ],
  [
    "d70692543810d4bf50d81cf44a55801a557a388a341367c7ea077ca306",
    "24DJEVBYCDKL6UGYDT2EUVMADJKXUOEKGQJWPR7KA56KGBQ=",
  ],
  [
    "6e08a89ca36b677ff8fe99e68a1241c8d8cef2570a5f60b6417d2538b30c",
    "NYEKRHFDNNTX76H6THTIUESBZDMM54SXBJPWBNSBPUSTRMYM",
  ],
  [
    "f2fc2319bd29457ccd01e8e194ee9bd7e97298b6610df4ab0f3d5baa0b2d7ccf69829edb74edef",
    "6L6CGGN5FFCXZTIB5DQZJ3U327UXFGFWMEG7JKYPHVN2UCZNPTHWTAU63N2O33Y=",
  ],
];

Deno.test({
  name: "[encoding.base32] encode",
  fn(): void {
    for (const [bin, b32] of testCases) {
      assertEquals(encode(fromHexString(bin)), b32);
    }
  },
});

Deno.test({
  name: "[encoding.base32] decode",
  fn(): void {
    for (const [bin, b32] of testCases) {
      assertEquals(toHexString(decode(b32)), bin);
    }
  },
});

Deno.test({
  name: "[encoding.base32] decode bad length",
  fn(): void {
    let errorCaught = false;
    try {
      decode("OOOO==");
    } catch (e) {
      assert(
        e.message.includes("Invalid string. Length must be a multiple of 8"),
      );
      errorCaught = true;
    }
    assert(errorCaught);
  },
});

Deno.test({
  name: "[encoding.base32] decode bad padding",
  fn(): void {
    let errorCaught = false;
    try {
      decode("OOOOOO==");
    } catch (e) {
      assert(e.message.includes("Invalid pad length"));
      errorCaught = true;
    }
    assert(errorCaught);
  },
});
