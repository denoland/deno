// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Fedor Indutny. All rights reserved. MIT license.

import { assertEquals } from "../../../../../testing/asserts.ts";
import { Buffer } from "../../../../buffer.ts";
import asn1 from "../mod.js";
const BN = asn1.bignum;

Deno.test("asn1.js PEM encoder/decoder", async function (t) {
  const model = asn1.define("Model", function () {
    this.seq().obj(
      this.key("a").int(),
      this.key("b").bitstr(),
      this.key("c").int(),
    );
  });

  const hundred = Buffer.alloc(100, "A");

  await t.step("should encode PEM", function () {
    const out = model.encode(
      {
        a: new BN(123),
        b: {
          data: hundred,
          unused: 0,
        },
        c: new BN(456),
      },
      "pem",
      {
        label: "MODEL",
      },
    );

    const expected = "-----BEGIN MODEL-----\n" +
      "MG4CAXsDZQBBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFB\n" +
      "QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFB\n" +
      "QUFBQUFBQUFBQUFBAgIByA==\n" +
      "-----END MODEL-----";
    assertEquals(out, expected);
  });

  await t.step("should decode PEM", function () {
    const expected = "-----BEGIN MODEL-----\n" +
      "MG4CAXsDZQBBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFB\n" +
      "QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFB\n" +
      "QUFBQUFBQUFBQUFBAgIByA==\n" +
      "-----END MODEL-----";

    const out = model.decode(expected, "pem", { label: "MODEL" });
    assertEquals(out.a.toString(), "123");
    assertEquals(out.b.data.toString(), hundred.toString());
    assertEquals(out.c.toString(), "456");
  });
});
