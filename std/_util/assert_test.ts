import { assert, AssertionError } from "../testing/asserts.ts";
import { assertThrows } from "https://deno.land/std@v0.50.0/testing/asserts.ts";
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
    }, AssertionError);
  },
});
test({
  name: "assert invalid scenario, with message",
  fn(): void {
    assertThrows(
      () => {
        assert(false, "Oops! Should be true");
      },
      AssertionError,
      "Oops! Should be true"
    );
  },
});
