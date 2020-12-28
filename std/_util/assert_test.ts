// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, DenoStdInternalError } from "./assert.ts";
import { assertThrows } from "../testing/asserts.ts";

Deno.test({
  name: "assert valid scenario",
  fn(): void {
    assert(true);
  },
});

Deno.test({
  name: "assert invalid scenario, no message",
  fn(): void {
    assertThrows(() => {
      assert(false);
    }, DenoStdInternalError);
  },
});
Deno.test({
  name: "assert invalid scenario, with message",
  fn(): void {
    assertThrows(
      () => {
        assert(false, "Oops! Should be true");
      },
      DenoStdInternalError,
      "Oops! Should be true",
    );
  },
});
