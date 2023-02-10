// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Fedor Indutny. All rights reserved. MIT license.

import { assertEquals } from "../../../../../testing/asserts.ts";
import { Buffer } from "../../../../buffer.ts";
import asn1 from "../mod.js";
import BN from "../../bn.js/bn.js";

Deno.test("asn1.js DER encoder", async function (t) {
  /*
   * Explicit value shold be wrapped with A0 | EXPLICIT tag
   * this adds two more bytes to resulting buffer.
   */
  await t.step("should code explicit tag as 0xA2", function () {
    const E = asn1.define("E", function () {
      this.explicit(2).octstr();
    });

    const encoded = E.encode("X", "der");

    // <Explicit tag> <wrapped len> <str tag> <len> <payload>
    assertEquals(encoded.toString("hex"), "a203040158");
    assertEquals(encoded.length, 5);
  });

  async function test(name, t, model_definition, model_value, der_expected) {
    await t.step(name, function () {
      const Model = asn1.define("Model", model_definition);
      const derActual = Model.encode(model_value, "der");
      assertEquals(derActual, Buffer.from(der_expected, "hex"));
    });
  }

  await test(
    "should encode objDesc",
    t,
    function () {
      this.objDesc();
    },
    Buffer.from("280"),
    "0703323830",
  );

  await test(
    "should encode choice",
    t,
    function () {
      this.choice({
        apple: this.bool(),
      });
    },
    { type: "apple", value: true },
    "0101ff",
  );

  await test(
    "should encode implicit seqof",
    t,
    function () {
      const Int = asn1.define("Int", function () {
        this.int();
      });
      this.implicit(0).seqof(Int);
    },
    [1],
    "A003020101",
  );

  await test(
    "should encode explicit seqof",
    t,
    function () {
      const Int = asn1.define("Int", function () {
        this.int();
      });
      this.explicit(0).seqof(Int);
    },
    [1],
    "A0053003020101",
  );

  await test(
    "should encode BN(128) properly",
    t,
    function () {
      this.int();
    },
    new BN(128),
    "02020080",
  );

  await test(
    "should encode int 128 properly",
    t,
    function () {
      this.int();
    },
    128,
    "02020080",
  );

  await test(
    "should encode 0x8011 properly",
    t,
    function () {
      this.int();
    },
    0x8011,
    "0203008011",
  );

  await test(
    "should omit default value in DER",
    t,
    function () {
      this.seq().obj(
        this.key("required").def(false).bool(),
        this.key("value").int(),
      );
    },
    { required: false, value: 1 },
    "3003020101",
  );

  await t.step("should encode optional and use", function () {
    const B = asn1.define("B", function () {
      this.int();
    });

    const A = asn1.define("A", function () {
      this.optional().use(B);
    });

    const out = A.encode(1, "der");
    assertEquals(out.toString("hex"), "020101");
  });

  await test(
    "should properly encode objid with dots",
    t,
    function () {
      this.objid({
        "1.2.398.3.10.1.1.1.2.2": "yes",
      });
    },
    "yes",
    "060a2a830e030a0101010202",
  );

  await test(
    "should properly encode objid as array of strings",
    t,
    function () {
      this.objid();
    },
    "1.2.398.3.10.1.1.1.2.2".split("."),
    "060a2a830e030a0101010202",
  );

  await test(
    "should properly encode bmpstr",
    t,
    function () {
      this.bmpstr();
    },
    "CertificateTemplate",
    "1e26004300650072007400690066006900630061" +
      "0074006500540065006d0070006c006100740065",
  );

  await test(
    "should properly encode bmpstr with cyrillic chars",
    t,
    function () {
      this.bmpstr();
    },
    "Привет",
    "1e0c041f04400438043204350442",
  );

  await t.step("should encode encapsulated models", function () {
    const B = asn1.define("B", function () {
      this.seq().obj(
        this.key("nested").int(),
      );
    });
    const A = asn1.define("A", function () {
      this.octstr().contains(B);
    });

    const out = A.encode({ nested: 5 }, "der");
    assertEquals(out.toString("hex"), "04053003020105");
  });

  await test(
    "should properly encode IA5 string",
    t,
    function () {
      this.ia5str();
    },
    "dog and bone",
    "160C646F6720616E6420626F6E65",
  );

  await test(
    "should properly encode printable string",
    t,
    function () {
      this.printstr();
    },
    "Brahms and Liszt",
    "1310427261686D7320616E64204C69737A74",
  );

  await test(
    "should properly encode T61 string",
    t,
    function () {
      this.t61str();
    },
    "Oliver Twist",
    "140C4F6C69766572205477697374",
  );

  await test(
    "should properly encode ISO646 string",
    t,
    function () {
      this.iso646str();
    },
    "septic tank",
    "1A0B7365707469632074616E6B",
  );

  await t.step("should not require encoder param", function () {
    const M = asn1.define("Model", function () {
      this.choice({
        apple: this.bool(),
      });
    });
    // Note no encoder specified, defaults to 'der'
    const encoded = M.encode({ "type": "apple", "value": true });
    assertEquals(encoded, Buffer.from("0101ff", "hex"));
  });
});
