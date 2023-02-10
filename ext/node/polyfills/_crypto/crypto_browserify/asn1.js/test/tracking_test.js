// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Fedor Indutny. All rights reserved. MIT license.

import { assertEquals } from "../../../../../testing/asserts.ts";
import asn1 from "../mod.js";

Deno.test("asn1.js tracking", async function (t) {
  await t.step("should track nested offsets", () => {
    const B = asn1.define("B", function () {
      this.seq().obj(
        this.key("x").int(),
        this.key("y").int(),
      );
    });

    const A = asn1.define("A", function () {
      this.seq().obj(
        this.key("a").explicit(0).use(B),
        this.key("b").use(B),
      );
    });

    const input = {
      a: { x: 1, y: 2 },
      b: { x: 3, y: 4 },
    };

    const tracked = [];

    const encoded = A.encode(input, "der");
    const _decoded = A.decode(encoded, "der", {
      track: function (path, start, end, type) {
        tracked.push([type, path, start, end]);
      },
    });

    assertEquals(tracked, [
      ["tagged", "", 0, 20],
      ["content", "", 2, 20],
      ["tagged", "a", 4, 12],
      ["content", "a", 6, 12],
      ["tagged", "a/x", 6, 9],
      ["content", "a/x", 8, 9],
      ["tagged", "a/y", 9, 12],
      ["content", "a/y", 11, 12],
      ["tagged", "b", 12, 20],
      ["content", "b", 14, 20],
      ["tagged", "b/x", 14, 17],
      ["content", "b/x", 16, 17],
      ["tagged", "b/y", 17, 20],
      ["content", "b/y", 19, 20],
    ]);
  });
});
