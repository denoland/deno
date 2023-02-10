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

Deno.test("BN.js/Reduction context", async function (t) {
  async function testMethod(name, fn) {
    await t.step(name + " method", async function (t) {
      await t.step(
        "should support add, iadd, sub, isub operations",
        function () {
          var p = new BN(257);
          var m = fn(p);
          var a = new BN(123).toRed(m);
          var b = new BN(231).toRed(m);

          assertEquals(a.redAdd(b).fromRed().toString(10), "97");
          assertEquals(a.redSub(b).fromRed().toString(10), "149");
          assertEquals(b.redSub(a).fromRed().toString(10), "108");

          assertEquals(a.clone().redIAdd(b).fromRed().toString(10), "97");
          assertEquals(a.clone().redISub(b).fromRed().toString(10), "149");
          assertEquals(b.clone().redISub(a).fromRed().toString(10), "108");
        },
      );

      await t.step("should support pow and mul operations", function () {
        var p192 = new BN(
          "fffffffffffffffffffffffffffffffeffffffffffffffff",
          16,
        );
        var m = fn(p192);
        var a = new BN(123);
        var b = new BN(231);
        var c = a.toRed(m).redMul(b.toRed(m)).fromRed();
        assert(c.cmp(a.mul(b).mod(p192)) === 0);

        assertEquals(
          a.toRed(m).redPow(new BN(0)).fromRed()
            .cmp(new BN(1)),
          0,
        );
        assertEquals(
          a.toRed(m).redPow(new BN(3)).fromRed()
            .cmp(a.sqr().mul(a)),
          0,
        );
        assertEquals(
          a.toRed(m).redPow(new BN(4)).fromRed()
            .cmp(a.sqr().sqr()),
          0,
        );
        assertEquals(
          a.toRed(m).redPow(new BN(8)).fromRed()
            .cmp(a.sqr().sqr().sqr()),
          0,
        );
        assertEquals(
          a.toRed(m).redPow(new BN(9)).fromRed()
            .cmp(a.sqr().sqr().sqr().mul(a)),
          0,
        );
        assertEquals(
          a.toRed(m).redPow(new BN(17)).fromRed()
            .cmp(a.sqr().sqr().sqr().sqr().mul(a)),
          0,
        );
        assertEquals(
          a.toRed(m).redPow(new BN("deadbeefabbadead", 16)).fromRed()
            .toString(16),
          "3aa0e7e304e320b68ef61592bcb00341866d6fa66e11a4d6",
        );
      });

      await t.step("should sqrtm numbers", function () {
        var p = new BN(263);
        var m = fn(p);
        var q = new BN(11).toRed(m);

        var qr = q.redSqrt();
        assertEquals(qr.redSqr().cmp(q), 0);

        qr = q.redSqrt();
        assertEquals(qr.redSqr().cmp(q), 0);

        p = new BN(
          "fffffffffffffffffffffffffffffffeffffffffffffffff",
          16,
        );
        m = fn(p);

        q = new BN(13).toRed(m);
        qr = q.redSqrt(true, p);
        assertEquals(qr.redSqr().cmp(q), 0);

        qr = q.redSqrt(false, p);
        assertEquals(qr.redSqr().cmp(q), 0);

        // Tonelli-shanks
        p = new BN(13);
        m = fn(p);
        q = new BN(10).toRed(m);
        assertEquals(q.redSqrt().fromRed().toString(10), "7");
      });

      await t.step("should invm numbers", function () {
        var p = new BN(257);
        var m = fn(p);
        var a = new BN(3).toRed(m);
        var b = a.redInvm();
        assertEquals(a.redMul(b).fromRed().toString(16), "1");
      });

      await t.step("should invm numbers (regression)", function () {
        var p = new BN(
          "ffffffff00000001000000000000000000000000ffffffffffffffffffffffff",
          16,
        );
        var a = new BN(
          "e1d969b8192fbac73ea5b7921896d6a2263d4d4077bb8e5055361d1f7f8163f3",
          16,
        );

        var m = fn(p);
        a = a.toRed(m);

        assertEquals(a.redInvm().fromRed().negative, 0);
      });

      await t.step("should imul numbers", function () {
        var p = new BN(
          "fffffffffffffffffffffffffffffffeffffffffffffffff",
          16,
        );
        var m = fn(p);

        var a = new BN("deadbeefabbadead", 16);
        var b = new BN("abbadeadbeefdead", 16);
        var c = a.mul(b).mod(p);

        assertEquals(
          a.toRed(m).redIMul(b.toRed(m)).fromRed().toString(16),
          c.toString(16),
        );
      });

      await t.step("should pow(base, 0) == 1", function () {
        var base = new BN(256).toRed(BN.red("k256"));
        var exponent = new BN(0);
        var result = base.redPow(exponent);
        assertEquals(result.toString(), "1");
      });

      await t.step("should shl numbers", function () {
        var base = new BN(256).toRed(BN.red("k256"));
        var result = base.redShl(1);
        assertEquals(result.toString(), "512");
      });

      await t.step("should reduce when converting to red", function () {
        var p = new BN(257);
        var m = fn(p);
        var a = new BN(5).toRed(m);

        var b = a.redISub(new BN(512).toRed(m));
        b.redISub(new BN(512).toRed(m));
      });

      await t.step("redNeg and zero value", function () {
        var a = new BN(0).toRed(BN.red("k256")).redNeg();
        assertEquals(a.isZero(), true);
      });

      await t.step("should not allow modulus <= 1", function () {
        assertThrows(function () {
          BN.red(new BN(0));
        }, /^Error: modulus must be greater than 1$/);

        assertThrows(function () {
          BN.red(new BN(1));
        }, /^Error: modulus must be greater than 1$/);

        BN.red(new BN(2));
      });
    });
  }

  await testMethod("Plain", BN.red);
  await testMethod("Montgomery", BN.mont);

  await t.step("Pseudo-Mersenne Primes", async function (t) {
    await t.step("should reduce numbers mod k256", function () {
      var p = BN._prime("k256");

      assertEquals(p.ireduce(new BN(0xdead)).toString(16), "dead");
      assertEquals(p.ireduce(new BN("deadbeef", 16)).toString(16), "deadbeef");

      var num = new BN(
        "fedcba9876543210fedcba9876543210dead" +
          "fedcba9876543210fedcba9876543210dead",
        16,
      );
      var exp = num.mod(p.p).toString(16);
      assertEquals(p.ireduce(num).toString(16), exp);

      var regr = new BN(
        "f7e46df64c1815962bf7bc9c56128798" +
          "3f4fcef9cb1979573163b477eab93959" +
          "335dfb29ef07a4d835d22aa3b6797760" +
          "70a8b8f59ba73d56d01a79af9",
        16,
      );
      exp = regr.mod(p.p).toString(16);

      assertEquals(p.ireduce(regr).toString(16), exp);
    });

    await t.step("should not fail to invm number mod k256", function () {
      var regr2 = new BN(
        "6c150c4aa9a8cf1934485d40674d4a7cd494675537bda36d49405c5d2c6f496f",
        16,
      );
      regr2 = regr2.toRed(BN.red("k256"));
      assertEquals(regr2.redInvm().redMul(regr2).fromRed().cmpn(1), 0);
    });

    await t.step("should correctly square the number", function () {
      var p = BN._prime("k256").p;
      var red = BN.red("k256");

      var n = new BN(
        "9cd8cb48c3281596139f147c1364a3ed" +
          "e88d3f310fdb0eb98c924e599ca1b3c9",
        16,
      );
      var expected = n.sqr().mod(p);
      var actual = n.toRed(red).redSqr().fromRed();

      assertEquals(actual.toString(16), expected.toString(16));
    });

    await t.step("redISqr should return right result", function () {
      var n = new BN("30f28939", 16);
      var actual = n.toRed(BN.red("k256")).redISqr().fromRed();
      assertEquals(actual.toString(16), "95bd93d19520eb1");
    });
  });

  await t.step("should avoid 4.1.0 regresion", function () {
    function bits2int(obits, q) {
      var bits = new BN(obits);
      var shift = (obits.length << 3) - q.bitLength();
      if (shift > 0) {
        bits.ishrn(shift);
      }
      return bits;
    }
    var t = Buffer.from(
      "aff1651e4cd6036d57aa8b2a05ccf1a9d5a40166340ecbbdc55" +
        "be10b568aa0aa3d05ce9a2fcec9df8ed018e29683c6051cb83e" +
        "46ce31ba4edb045356a8d0d80b",
      "hex",
    );
    var g = new BN(
      "5c7ff6b06f8f143fe8288433493e4769c4d988ace5be25a0e24809670" +
        "716c613d7b0cee6932f8faa7c44d2cb24523da53fbe4f6ec3595892d1" +
        "aa58c4328a06c46a15662e7eaa703a1decf8bbb2d05dbe2eb956c142a" +
        "338661d10461c0d135472085057f3494309ffa73c611f78b32adbb574" +
        "0c361c9f35be90997db2014e2ef5aa61782f52abeb8bd6432c4dd097b" +
        "c5423b285dafb60dc364e8161f4a2a35aca3a10b1c4d203cc76a470a3" +
        "3afdcbdd92959859abd8b56e1725252d78eac66e71ba9ae3f1dd24871" +
        "99874393cd4d832186800654760e1e34c09e4d155179f9ec0dc4473f9" +
        "96bdce6eed1cabed8b6f116f7ad9cf505df0f998e34ab27514b0ffe7",
      16,
    );
    var p = new BN(
      "9db6fb5951b66bb6fe1e140f1d2ce5502374161fd6538df1648218642" +
        "f0b5c48c8f7a41aadfa187324b87674fa1822b00f1ecf8136943d7c55" +
        "757264e5a1a44ffe012e9936e00c1d3e9310b01c7d179805d3058b2a9" +
        "f4bb6f9716bfe6117c6b5b3cc4d9be341104ad4a80ad6c94e005f4b99" +
        "3e14f091eb51743bf33050c38de235567e1b34c3d6a5c0ceaa1a0f368" +
        "213c3d19843d0b4b09dcb9fc72d39c8de41f1bf14d4bb4563ca283716" +
        "21cad3324b6a2d392145bebfac748805236f5ca2fe92b871cd8f9c36d" +
        "3292b5509ca8caa77a2adfc7bfd77dda6f71125a7456fea153e433256" +
        "a2261c6a06ed3693797e7995fad5aabbcfbe3eda2741e375404ae25b",
      16,
    );
    var q = new BN(
      "f2c3119374ce76c9356990b465374a17f23f9ed35089bd969f61c6dde" +
        "9998c1f",
      16,
    );
    var k = bits2int(t, q);
    var expectedR = "89ec4bb1400eccff8e7d9aa515cd1de7803f2daff09693ee7fd1353e" +
      "90a68307";
    var r = g.toRed(BN.mont(p)).redPow(k).fromRed().mod(q);
    assertEquals(r.toString(16), expectedR);
  });

  await t.step(
    "K256.split for 512 bits number should return equal numbers",
    function () {
      var red = BN.red("k256");
      var input = new BN(1).iushln(512).subn(1);
      assertEquals(input.bitLength(), 512);
      var output = new BN(0);
      red.prime.split(input, output);
      assertEquals(input.cmp(output), 0);
    },
  );

  await t.step("imod should change host object", function () {
    var red = BN.red(new BN(13));
    var a = new BN(2).toRed(red);
    var b = new BN(7).toRed(red);
    var c = a.redIMul(b);
    assertEquals(a.toNumber(), 1);
    assertEquals(c.toNumber(), 1);
  });
});
