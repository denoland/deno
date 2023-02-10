// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2015 Fedor Indutny. All rights reserved. MIT license.
// deno-lint-ignore-file no-var

import {
  assert,
  assertEquals,
  assertThrows,
} from "../../../../testing/asserts.ts";
import { Buffer } from "../../../buffer.ts";
import { BN } from "./bn.js";

Deno.test("BN.js/Constructor", async function (t) {
  await t.step("with Smi input", async function (t) {
    await t.step("should accept one limb number", function () {
      assertEquals(new BN(12345).toString(16), "3039");
    });

    await t.step("should accept two-limb number", function () {
      assertEquals(new BN(0x4123456).toString(16), "4123456");
    });

    await t.step("should accept 52 bits of precision", function () {
      var num = Math.pow(2, 52);
      assertEquals(new BN(num, 10).toString(10), num.toString(10));
    });

    await t.step("should accept max safe integer", function () {
      var num = Math.pow(2, 53) - 1;
      assertEquals(new BN(num, 10).toString(10), num.toString(10));
    });

    await t.step("should not accept an unsafe integer", function () {
      var num = Math.pow(2, 53);

      assertThrows(function () {
        return new BN(num, 10);
      }, /^Error: Assertion failed$/);
    });

    await t.step("should accept two-limb LE number", function () {
      assertEquals(new BN(0x4123456, null, "le").toString(16), "56341204");
    });
  });

  await t.step("with String input", async function (t) {
    await t.step("should accept base-16", function () {
      assertEquals(new BN("1A6B765D8CDF", 16).toString(16), "1a6b765d8cdf");
      assertEquals(new BN("1A6B765D8CDF", 16).toString(), "29048849665247");
    });

    await t.step("should accept base-hex", function () {
      assertEquals(new BN("FF", "hex").toString(), "255");
    });

    await t.step("should accept base-16 with spaces", function () {
      var num = "a89c e5af8724 c0a23e0e 0ff77500";
      assertEquals(new BN(num, 16).toString(16), num.replace(/ /g, ""));
    });

    await t.step("should accept long base-16", function () {
      var num = "123456789abcdef123456789abcdef123456789abcdef";
      assertEquals(new BN(num, 16).toString(16), num);
    });

    await t.step("should accept positive base-10", function () {
      assertEquals(new BN("10654321").toString(), "10654321");
      assertEquals(new BN("29048849665247").toString(16), "1a6b765d8cdf");
    });

    await t.step("should accept negative base-10", function () {
      assertEquals(new BN("-29048849665247").toString(16), "-1a6b765d8cdf");
    });

    await t.step("should accept long base-10", function () {
      var num = "10000000000000000";
      assertEquals(new BN(num).toString(10), num);
    });

    await t.step("should accept base-2", function () {
      var base2 = "11111111111111111111111111111111111111111111111111111";
      assertEquals(new BN(base2, 2).toString(2), base2);
    });

    await t.step("should accept base-36", function () {
      var base36 = "zzZzzzZzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";
      assertEquals(new BN(base36, 36).toString(36), base36.toLowerCase());
    });

    await t.step("should not overflow limbs during base-10", function () {
      var num = "65820182292848241686198767302293" +
        "20890292528855852623664389292032";
      assert(new BN(num).words[0] < 0x4000000);
    });

    await t.step("should accept base-16 LE integer", function () {
      assertEquals(
        new BN("1A6B765D8CDF", 16, "le").toString(16),
        "df8c5d766b1a",
      );
    });

    await t.step(
      "should accept base-16 LE integer with leading zeros",
      function () {
        assertEquals(new BN("0010", 16, "le").toNumber(), 4096);
        assertEquals(new BN("-010", 16, "le").toNumber(), -4096);
        assertEquals(new BN("010", 16, "le").toNumber(), 4096);
      },
    );

    await t.step("should not accept wrong characters for base", function () {
      assertThrows(function () {
        return new BN("01FF");
      }, /^Error: Invalid character$/);
    });

    await t.step("should not accept decimal", function () {
      assertThrows(function () {
        new BN("10.00", 10); // eslint-disable-line no-new
      }, /Invalid character/);

      assertThrows(function () {
        new BN("16.00", 16); // eslint-disable-line no-new
      }, /Invalid character/);
    });

    await t.step("should not accept non-hex characters", function () {
      [
        "0000000z",
        "000000gg",
        "0000gg00",
        "fffggfff",
        "/0000000",
        "0-000000", // if -, is first, that is OK
        "ff.fffff",
        "hexadecimal",
      ].forEach(function (str) {
        assertThrows(function () {
          new BN(str, 16); // eslint-disable-line no-new
        }, /Invalid character in /);
      });
    });
  });

  await t.step("with Array input", async function (t) {
    await t.step("should not fail on empty array", function () {
      assertEquals(new BN([]).toString(16), "0");
    });

    await t.step("should import/export big endian", function () {
      assertEquals(new BN([0, 1], 16).toString(16), "1");
      assertEquals(new BN([1, 2, 3]).toString(16), "10203");
      assertEquals(new BN([1, 2, 3, 4]).toString(16), "1020304");
      assertEquals(new BN([1, 2, 3, 4, 5]).toString(16), "102030405");
      assertEquals(
        new BN([1, 2, 3, 4, 5, 6, 7, 8]).toString(16),
        "102030405060708",
      );
      assertEquals(new BN([1, 2, 3, 4]).toArray().join(","), "1,2,3,4");
      assertEquals(
        new BN([1, 2, 3, 4, 5, 6, 7, 8]).toArray().join(","),
        "1,2,3,4,5,6,7,8",
      );
    });

    await t.step("should import little endian", function () {
      assertEquals(new BN([0, 1], 16, "le").toString(16), "100");
      assertEquals(new BN([1, 2, 3], 16, "le").toString(16), "30201");
      assertEquals(new BN([1, 2, 3, 4], 16, "le").toString(16), "4030201");
      assertEquals(new BN([1, 2, 3, 4, 5], 16, "le").toString(16), "504030201");
      assertEquals(
        new BN([1, 2, 3, 4, 5, 6, 7, 8], "le").toString(16),
        "807060504030201",
      );
      assertEquals(new BN([1, 2, 3, 4]).toArray("le").join(","), "4,3,2,1");
      assertEquals(
        new BN([1, 2, 3, 4, 5, 6, 7, 8]).toArray("le").join(","),
        "8,7,6,5,4,3,2,1",
      );
    });

    await t.step("should import big endian with implicit base", function () {
      assertEquals(new BN([1, 2, 3, 4, 5], "le").toString(16), "504030201");
    });
  });

  // the Array code is able to handle Buffer
  await t.step("with Buffer input", async function (t) {
    await t.step("should not fail on empty Buffer", function () {
      assertEquals(new BN(Buffer.alloc(0)).toString(16), "0");
    });

    await t.step("should import/export big endian", function () {
      assertEquals(new BN(Buffer.from("010203", "hex")).toString(16), "10203");
    });

    await t.step("should import little endian", function () {
      assertEquals(
        new BN(Buffer.from("010203", "hex"), "le").toString(16),
        "30201",
      );
    });
  });

  await t.step("with BN input", async function (t) {
    await t.step("should clone BN", function () {
      var num = new BN(12345);
      assertEquals(new BN(num).toString(10), "12345");
    });
  });
});
