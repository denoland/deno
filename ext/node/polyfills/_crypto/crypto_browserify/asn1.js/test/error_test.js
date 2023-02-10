// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Fedor Indutny. All rights reserved. MIT license.

import {
  assert,
  assertEquals,
  assertThrows,
} from "../../../../../testing/asserts.ts";
import { Buffer } from "../../../../buffer.ts";
import asn1 from "../mod.js";
import { jsonEqual } from "./util.js";
const bn = asn1.bignum;

Deno.test("asn1.js error", async function (t) {
  await t.step("encoder", async function (t) {
    async function test(name, t, model, input, expected) {
      await t.step("should support " + name, function () {
        const M = asn1.define("TestModel", model);

        let error;
        assertThrows(function () {
          try {
            const _encoded = M.encode(input, "der");
          } catch (e) {
            error = e;
            throw e;
          }
        });

        assert(
          expected.test(error.stack),
          "Failed to match, expected: " + expected + " got: " +
            JSON.stringify(error.stack),
        );
      });
    }

    await t.step("primitives", async function (t) {
      await test(
        "int",
        t,
        function () {
          this.int();
        },
        "hello",
        /no values map/i,
      );

      await test(
        "enum",
        t,
        function () {
          this.enum({ 0: "hello", 1: "world" });
        },
        "gosh",
        /contain: "gosh"/,
      );

      await test(
        "objid",
        t,
        function () {
          this.objid();
        },
        1,
        /objid\(\) should be either array or string, got: 1/,
      );

      await test(
        "numstr",
        t,
        function () {
          this.numstr();
        },
        "hello",
        /only digits and space/,
      );

      await test(
        "printstr",
        t,
        function () {
          this.printstr();
        },
        "hello!",
        /only latin upper and lower case letters/,
      );
    });

    await t.step("composite", async function (t) {
      await test(
        "shallow",
        t,
        function () {
          this.seq().obj(
            this.key("key").int(),
          );
        },
        { key: "hello" },
        /map at: \["key"\]/i,
      );

      await test(
        "deep and empty",
        t,
        function () {
          this.seq().obj(
            this.key("a").seq().obj(
              this.key("b").seq().obj(
                this.key("c").int(),
              ),
            ),
          );
        },
        {},
        /input is not object at: \["a"\]\["b"\]/i,
      );

      await test(
        "deep",
        t,
        function () {
          this.seq().obj(
            this.key("a").seq().obj(
              this.key("b").seq().obj(
                this.key("c").int(),
              ),
            ),
          );
        },
        { a: { b: { c: "hello" } } },
        /map at: \["a"\]\["b"\]\["c"\]/i,
      );

      await test(
        "use",
        t,
        function () {
          const S = asn1.define("S", function () {
            this.seq().obj(
              this.key("x").int(),
            );
          });

          this.seq().obj(
            this.key("a").seq().obj(
              this.key("b").use(S),
            ),
          );
        },
        { a: { b: { x: "hello" } } },
        /map at: \["a"\]\["b"\]\["x"\]/i,
      );
    });
  });

  await t.step("decoder", async function (t) {
    async function test(name, t, model, input, expected) {
      await t.step("should support " + name, function () {
        const M = asn1.define("TestModel", model);

        let error;
        assertThrows(function () {
          try {
            const _decoded = M.decode(Buffer.from(input, "hex"), "der");
          } catch (e) {
            error = e;
            throw e;
          }
        });
        const partial = M.decode(Buffer.from(input, "hex"), "der", {
          partial: true,
        });

        assert(
          expected.test(error.stack),
          "Failed to match, expected: " + expected + " got: " +
            JSON.stringify(error.stack),
        );

        assertEquals(partial.errors.length, 1);
        assert(
          expected.test(partial.errors[0].stack),
          "Failed to match, expected: " + expected + " got: " +
            JSON.stringify(partial.errors[0].stack),
        );
      });
    }

    await t.step("primitive", async function (t) {
      await test(
        "int",
        t,
        function () {
          this.int();
        },
        "2201",
        /body of: "int"/,
      );

      await test(
        "int",
        t,
        function () {
          this.int();
        },
        "",
        /tag of "int"/,
      );

      await test(
        "bmpstr invalid length",
        t,
        function () {
          this.bmpstr();
        },
        "1e0b041f04400438043204350442",
        /bmpstr length mismatch/,
      );

      await test(
        "numstr unsupported characters",
        t,
        function () {
          this.numstr();
        },
        "12024141",
        /numstr unsupported characters/,
      );

      await test(
        "printstr unsupported characters",
        t,
        function () {
          this.printstr();
        },
        "13024121",
        /printstr unsupported characters/,
      );
    });

    await t.step("composite", async function (t) {
      await test(
        "shallow",
        t,
        function () {
          this.seq().obj(
            this.key("a").seq().obj(),
          );
        },
        "30",
        /length of "seq"/,
      );

      await test(
        "deep and empty",
        t,
        function () {
          this.seq().obj(
            this.key("a").seq().obj(
              this.key("b").seq().obj(
                this.key("c").int(),
              ),
            ),
          );
        },
        "300430023000",
        /tag of "int" at: \["a"\]\["b"\]\["c"\]/,
      );

      await test(
        "deep and incomplete",
        t,
        function () {
          this.seq().obj(
            this.key("a").seq().obj(
              this.key("b").seq().obj(
                this.key("c").int(),
              ),
            ),
          );
        },
        "30053003300122",
        /length of "int" at: \["a"\]\["b"\]\["c"\]/,
      );
    });
  });

  await t.step("partial decoder", async function (t) {
    async function test(name, t, model, input, expectedObj, expectedErrs) {
      await t.step("should support " + name, function () {
        const M = asn1.define("TestModel", model);

        const decoded = M.decode(Buffer.from(input, "hex"), "der", {
          partial: true,
        });

        jsonEqual(decoded.result, expectedObj);

        assertEquals(decoded.errors.length, expectedErrs.length);
        expectedErrs.forEach(function (expected, i) {
          assert(
            expected.test(decoded.errors[i].stack),
            "Failed to match, expected: " + expected + " got: " +
              JSON.stringify(decoded.errors[i].stack),
          );
        });
      });
    }

    await test(
      "last key not present",
      t,
      function () {
        this.seq().obj(
          this.key("a").seq().obj(
            this.key("b").seq().obj(
              this.key("c").int(),
            ),
            this.key("d").int(),
          ),
        );
      },
      "30073005300022012e",
      { a: { b: {}, d: new bn(46) } },
      [
        /"int" at: \["a"\]\["b"\]\["c"\]/,
      ],
    );

    await test(
      "first key not present",
      t,
      function () {
        this.seq().obj(
          this.key("a").seq().obj(
            this.key("b").seq().obj(
              this.key("c").int(),
            ),
            this.key("d").int(),
          ),
        );
      },
      "30073005300322012e",
      { a: { b: { c: new bn(46) } } },
      [
        /"int" at: \["a"\]\["d"\]/,
      ],
    );
  });
});
