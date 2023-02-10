// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Fedor Indutny. All rights reserved. MIT license.

import { assert, assertEquals } from "../../../../../testing/asserts.ts";
import asn1 from "../mod.js";
import { jsonEqual } from "./util.js";
import { Buffer } from "../../../../buffer.ts";

const bn = asn1.bignum;

Deno.test("asn1.js models", async function (t) {
  await t.step("plain use", async function (t) {
    await t.step("should encode submodel", function () {
      const SubModel = asn1.define("SubModel", function () {
        this.seq().obj(
          this.key("b").octstr(),
        );
      });
      const Model = asn1.define("Model", function () {
        this.seq().obj(
          this.key("a").int(),
          this.key("sub").use(SubModel),
        );
      });

      const data = { a: new bn(1), sub: { b: Buffer.from("XXX") } };
      const wire = Model.encode(data, "der");
      assertEquals(wire.toString("hex"), "300a02010130050403585858");
      const back = Model.decode(wire, "der");
      jsonEqual(back, data);
    });

    await t.step("should honour implicit tag from parent", function () {
      const SubModel = asn1.define("SubModel", function () {
        this.seq().obj(
          this.key("x").octstr(),
        );
      });
      const Model = asn1.define("Model", function () {
        this.seq().obj(
          this.key("a").int(),
          this.key("sub").use(SubModel).implicit(0),
        );
      });

      const data = { a: new bn(1), sub: { x: Buffer.from("123") } };
      const wire = Model.encode(data, "der");
      assertEquals(wire.toString("hex"), "300a020101a0050403313233");
      const back = Model.decode(wire, "der");
      jsonEqual(back, data);
    });

    await t.step("should honour explicit tag from parent", function () {
      const SubModel = asn1.define("SubModel", function () {
        this.seq().obj(
          this.key("x").octstr(),
        );
      });
      const Model = asn1.define("Model", function () {
        this.seq().obj(
          this.key("a").int(),
          this.key("sub").use(SubModel).explicit(0),
        );
      });

      const data = { a: new bn(1), sub: { x: Buffer.from("123") } };
      const wire = Model.encode(data, "der");
      assertEquals(wire.toString("hex"), "300c020101a00730050403313233");
      const back = Model.decode(wire, "der");
      jsonEqual(back, data);
    });

    await t.step("should get model with function call", function () {
      const SubModel = asn1.define("SubModel", function () {
        this.seq().obj(
          this.key("x").octstr(),
        );
      });
      const Model = asn1.define("Model", function () {
        this.seq().obj(
          this.key("a").int(),
          this.key("sub").use(function (obj) {
            assert(obj.a == 1);
            return SubModel;
          }),
        );
      });

      const data = { a: new bn(1), sub: { x: Buffer.from("123") } };
      const wire = Model.encode(data, "der");
      assertEquals(wire.toString("hex"), "300a02010130050403313233");
      const back = Model.decode(wire, "der");
      jsonEqual(back, data);
    });

    await t.step("should support recursive submodels", function () {
      const PlainSubModel = asn1.define("PlainSubModel", function () {
        this.int();
      });
      const RecursiveModel = asn1.define("RecursiveModel", function () {
        this.seq().obj(
          this.key("plain").bool(),
          this.key("content").use(function (obj) {
            if (obj.plain) {
              return PlainSubModel;
            } else {
              return RecursiveModel;
            }
          }),
        );
      });

      const data = {
        "plain": false,
        "content": {
          "plain": true,
          "content": new bn(1),
        },
      };
      const wire = RecursiveModel.encode(data, "der");
      assertEquals(wire.toString("hex"), "300b01010030060101ff020101");
      const back = RecursiveModel.decode(wire, "der");
      jsonEqual(back, data);
    });
  });
});
