// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Fedor Indutny. All rights reserved. MIT license.

import { Buffer } from "../../../../buffer.ts";
import asn1 from "../mod.js";
import { jsonEqual } from "./util.js";

Deno.test("asn1.js ping/pong", async function (t) {
  async function test(name, t, model, input, expected) {
    await t.step("should support " + name, function () {
      const M = asn1.define("TestModel", model);

      const encoded = M.encode(input, "der");
      const decoded = M.decode(encoded, "der");
      jsonEqual(decoded, expected !== undefined ? expected : input);
    });
  }

  await t.step("primitives", async function (t) {
    await test("bigint", t, function () {
      this.int();
    }, new asn1.bignum("0102030405060708", 16));

    await test("enum", t, function () {
      this.enum({ 0: "hello", 1: "world" });
    }, "world");

    await test("octstr", t, function () {
      this.octstr();
    }, Buffer.from("hello"));

    await test("objDesc", t, function () {
      this.objDesc();
    }, Buffer.from("hello"));

    await test("bitstr", t, function () {
      this.bitstr();
    }, { unused: 4, data: Buffer.from("hello!") });

    await test("ia5str", t, function () {
      this.ia5str();
    }, "hello");

    await test("utf8str", t, function () {
      this.utf8str();
    }, "hello");

    await test("bmpstr", t, function () {
      this.bmpstr();
    }, "hello");

    await test("numstr", t, function () {
      this.numstr();
    }, "1234 5678 90");

    await test("printstr", t, function () {
      this.printstr();
    }, "hello");

    await test("gentime", t, function () {
      this.gentime();
    }, 1385921175000);

    await test(
      "gentime 0",
      t,
      function () {
        this.gentime();
      },
      0,
      0,
    );

    await test("utctime", t, function () {
      this.utctime();
    }, 1385921175000);

    await test(
      "utctime 0",
      t,
      function () {
        this.utctime();
      },
      0,
      0,
    );

    await test("utctime regression", t, function () {
      this.utctime();
    }, 1414454400000);

    await test("null", t, function () {
      this.null_();
    }, null);

    await test("objid", t, function () {
      this.objid({
        "1 3 6 1 5 5 7 48 1 1": "id-pkix-ocsp-basic",
      });
    }, "id-pkix-ocsp-basic");

    await test("true", t, function () {
      this.bool();
    }, true);

    await test("false", t, function () {
      this.bool();
    }, false);

    await test(
      "any",
      t,
      function () {
        this.any();
      },
      Buffer.from(
        "02210081347a0d3d674aeeb563061d94a3aea5f6a7" +
          "c6dc153ea90a42c1ca41929ac1b9",
        "hex",
      ),
    );

    await test(
      "default explicit",
      t,
      function () {
        this.seq().obj(
          this.key("version").def("v1").explicit(0).int({
            0: "v1",
            1: "v2",
          }),
        );
      },
      {},
      { "version": "v1" },
    );

    await test(
      "implicit",
      t,
      function () {
        this.implicit(0).int({
          0: "v1",
          1: "v2",
        });
      },
      "v2",
      "v2",
    );
  });

  await t.step("composite", async function (t) {
    await test(
      "2x int",
      t,
      function () {
        this.seq().obj(
          this.key("hello").int(),
          this.key("world").int(),
        );
      },
      { hello: 4, world: 2 },
      { hello: "04", world: "02" },
    );

    await test("enum", t, function () {
      this.seq().obj(
        this.key("hello").enum({ 0: "world", 1: "devs" }),
      );
    }, { hello: "devs" });

    await test("optionals", t, function () {
      this.seq().obj(
        this.key("hello").enum({ 0: "world", 1: "devs" }),
        this.key("how").optional().def("are you").enum({
          0: "are you",
          1: "are we?!",
        }),
      );
    }, { hello: "devs", how: "are we?!" });

    await test(
      "optionals #2",
      t,
      function () {
        this.seq().obj(
          this.key("hello").enum({ 0: "world", 1: "devs" }),
          this.key("how").optional().def("are you").enum({
            0: "are you",
            1: "are we?!",
          }),
        );
      },
      { hello: "devs" },
      { hello: "devs", how: "are you" },
    );

    await test(
      "optionals #3",
      t,
      function () {
        this.seq().obj(
          this.key("content").optional().int(),
        );
      },
      {},
      {},
    );

    await test("optional + any", t, function () {
      this.seq().obj(
        this.key("content").optional().any(),
      );
    }, { content: Buffer.from("0500", "hex") });

    await test(
      "seqof",
      t,
      function () {
        const S = asn1.define("S", function () {
          this.seq().obj(
            this.key("a").def("b").int({ 0: "a", 1: "b" }),
            this.key("c").def("d").int({ 2: "c", 3: "d" }),
          );
        });
        this.seqof(S);
      },
      [{}, { a: "a", c: "c" }],
      [{ a: "b", c: "d" }, { a: "a", c: "c" }],
    );

    await test("choice", t, function () {
      this.choice({
        apple: this.bool(),
      });
    }, { type: "apple", value: true });
  });
});
