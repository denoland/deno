// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert } from "../assert/mod.ts";
import { isValidOperator } from "./_shared.ts";

Deno.test({
  name: "valid_operators",
  fn: async (t) => {
    const operators: unknown[] = [
      "",
      "=",
      "==",
      "===",
      "!=",
      "!==",
      ">",
      ">=",
      "<",
      "<=",
    ];
    for (const op of operators) {
      await t.step(`valid operator ${op}`, () => {
        const actual = isValidOperator(op);
        assert(actual);
      });
    }
  },
});
