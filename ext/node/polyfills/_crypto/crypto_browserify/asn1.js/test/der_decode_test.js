// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Fedor Indutny. All rights reserved. MIT license.

import { assertEquals } from "../../../../../testing/asserts.ts";
import { Buffer } from "../../../../buffer.ts";
import asn1 from "../mod.js";

Deno.test("asn1.js DER decoder", async function (t) {
  await t.step("should propagate implicit tag", function () {
    const B = asn1.define("B", function () {
      this.seq().obj(
        this.key("b").octstr(),
      );
    });

    const A = asn1.define("Bug", function () {
      this.seq().obj(
        this.key("a").implicit(0).use(B),
      );
    });

    const out = A.decode(Buffer.from("300720050403313233", "hex"), "der");
    assertEquals(out.a.b.toString(), "123");
  });

  await t.step("should decode optional tag to undefined key", function () {
    const A = asn1.define("A", function () {
      this.seq().obj(
        this.key("key").bool(),
        this.optional().key("opt").bool(),
      );
    });
    const out = A.decode(Buffer.from("30030101ff", "hex"), "der");
    assertEquals(out, { "key": true });
  });

  await t.step("should decode optional tag to default value", function () {
    const A = asn1.define("A", function () {
      this.seq().obj(
        this.key("key").bool(),
        this.optional().key("opt").octstr().def("default"),
      );
    });
    const out = A.decode(Buffer.from("30030101ff", "hex"), "der");
    assertEquals(out, { "key": true, "opt": "default" });
  });

  async function test(name, t, model, inputHex, expected) {
    await t.step(name, function () {
      const M = asn1.define("Model", model);
      const decoded = M.decode(Buffer.from(inputHex, "hex"), "der");
      assertEquals(decoded, expected);
    });
  }

  await test(
    "should decode choice",
    t,
    function () {
      this.choice({
        apple: this.bool(),
      });
    },
    "0101ff",
    { "type": "apple", "value": true },
  );

  await t.step("should decode optional and use", function () {
    const B = asn1.define("B", function () {
      this.int();
    });

    const A = asn1.define("A", function () {
      this.optional().use(B);
    });

    const out = A.decode(Buffer.from("020101", "hex"), "der");
    assertEquals(out.toString(10), "1");
  });

  await test(
    "should decode indefinite length",
    t,
    function () {
      this.seq().obj(
        this.key("key").bool(),
      );
    },
    "30800101ff0000",
    { "key": true },
  );

  await test(
    "should decode objDesc",
    t,
    function () {
      this.objDesc();
    },
    "0703323830",
    Buffer.from("280"),
  );

  await test(
    "should decode bmpstr",
    t,
    function () {
      this.bmpstr();
    },
    "1e26004300650072007400690066006900630061" +
      "0074006500540065006d0070006c006100740065",
    "CertificateTemplate",
  );

  await test(
    "should decode bmpstr with cyrillic chars",
    t,
    function () {
      this.bmpstr();
    },
    "1e0c041f04400438043204350442",
    "Привет",
  );

  await test(
    "should properly decode objid with dots",
    t,
    function () {
      this.objid({
        "1.2.398.3.10.1.1.1.2.2": "yes",
      });
    },
    "060a2a830e030a0101010202",
    "yes",
  );

  await t.step("should decode encapsulated models", function () {
    const B = asn1.define("B", function () {
      this.seq().obj(
        this.key("nested").int(),
      );
    });
    const A = asn1.define("A", function () {
      this.octstr().contains(B);
    });

    const out = A.decode(Buffer.from("04053003020105", "hex"), "der");
    assertEquals(out.nested.toString(10), "5");
  });

  await test(
    "should decode IA5 string",
    t,
    function () {
      this.ia5str();
    },
    "160C646F6720616E6420626F6E65",
    "dog and bone",
  );

  await test(
    "should decode printable string",
    t,
    function () {
      this.printstr();
    },
    "1310427261686D7320616E64204C69737A74",
    "Brahms and Liszt",
  );

  await test(
    "should decode T61 string",
    t,
    function () {
      this.t61str();
    },
    "140C4F6C69766572205477697374",
    "Oliver Twist",
  );

  await test(
    "should decode ISO646 string",
    t,
    function () {
      this.iso646str();
    },
    "1A0B7365707469632074616E6B",
    "septic tank",
  );

  await t.step("should decode optional seqof", function () {
    const B = asn1.define("B", function () {
      this.seq().obj(
        this.key("num").int(),
      );
    });
    const A = asn1.define("A", function () {
      this.seq().obj(
        this.key("test1").seqof(B),
        this.key("test2").optional().seqof(B),
      );
    });

    let out = A.decode(
      Buffer.from(
        "3018300A30030201013003020102300A30030201033003020104",
        "hex",
      ),
      "der",
    );
    assertEquals(out.test1[0].num.toString(10), "1");
    assertEquals(out.test1[1].num.toString(10), "2");
    assertEquals(out.test2[0].num.toString(10), "3");
    assertEquals(out.test2[1].num.toString(10), "4");

    out = A.decode(Buffer.from("300C300A30030201013003020102", "hex"), "der");
    assertEquals(out.test1[0].num.toString(10), "1");
    assertEquals(out.test1[1].num.toString(10), "2");
    assertEquals(out.test2, undefined);
  });

  await t.step("should not require decoder param", function () {
    const M = asn1.define("Model", function () {
      this.choice({
        apple: this.bool(),
      });
    });
    // Note no decoder specified, defaults to 'der'
    const decoded = M.decode(Buffer.from("0101ff", "hex"));
    assertEquals(decoded, { "type": "apple", "value": true });
  });
});
