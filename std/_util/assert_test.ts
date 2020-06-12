import { assert, DenoStdInternalError } from "./assert.ts";
import { assertThrows } from "../testing/asserts.ts";

const { test } = Deno;

test({
  name: "assert valid scenario",
  fn(): void {
    assert(true);
  },
});

test({
  name: "assert invalid scenario, no message",
  fn(): void {
    assertThrows(() => {
      assert(false);
    }, DenoStdInternalError);
  },
});
test({
  name: "assert invalid scenario, with message",
  fn(): void {
    assertThrows(
      () => {
        assert(false, "Oops! Should be true");
      },
      DenoStdInternalError,
      "Oops! Should be true"
    );
  },
});
