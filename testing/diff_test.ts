import diff from "./diff.ts";
import { test, assertEqual } from "./mod.ts";

test({
  name: "empty",
  fn() {
    assertEqual(diff([], []), []);
  }
});

test({
  name: '"a" vs "b"',
  fn() {
    assertEqual(diff(["a"], ["b"]), [
      { type: "removed", value: "a" },
      { type: "added", value: "b" }
    ]);
  }
});

test({
  name: '"a" vs "a"',
  fn() {
    assertEqual(diff(["a"], ["a"]), [{ type: "common", value: "a" }]);
  }
});

test({
  name: '"a" vs ""',
  fn() {
    assertEqual(diff(["a"], []), [{ type: "removed", value: "a" }]);
  }
});

test({
  name: '"" vs "a"',
  fn() {
    assertEqual(diff([], ["a"]), [{ type: "added", value: "a" }]);
  }
});

test({
  name: '"a" vs "a, b"',
  fn() {
    assertEqual(diff(["a"], ["a", "b"]), [
      { type: "common", value: "a" },
      { type: "added", value: "b" }
    ]);
  }
});

test({
  name: '"strength" vs "string"',
  fn() {
    assertEqual(diff(Array.from("strength"), Array.from("string")), [
      { type: "common", value: "s" },
      { type: "common", value: "t" },
      { type: "common", value: "r" },
      { type: "removed", value: "e" },
      { type: "added", value: "i" },
      { type: "common", value: "n" },
      { type: "common", value: "g" },
      { type: "removed", value: "t" },
      { type: "removed", value: "h" }
    ]);
  }
});

test({
  name: '"strength" vs ""',
  fn() {
    assertEqual(diff(Array.from("strength"), Array.from("")), [
      { type: "removed", value: "s" },
      { type: "removed", value: "t" },
      { type: "removed", value: "r" },
      { type: "removed", value: "e" },
      { type: "removed", value: "n" },
      { type: "removed", value: "g" },
      { type: "removed", value: "t" },
      { type: "removed", value: "h" }
    ]);
  }
});

test({
  name: '"" vs "strength"',
  fn() {
    assertEqual(diff(Array.from(""), Array.from("strength")), [
      { type: "added", value: "s" },
      { type: "added", value: "t" },
      { type: "added", value: "r" },
      { type: "added", value: "e" },
      { type: "added", value: "n" },
      { type: "added", value: "g" },
      { type: "added", value: "t" },
      { type: "added", value: "h" }
    ]);
  }
});

test({
  name: '"abc", "c" vs "abc", "bcd", "c"',
  fn() {
    assertEqual(diff(["abc", "c"], ["abc", "bcd", "c"]), [
      { type: "common", value: "abc" },
      { type: "added", value: "bcd" },
      { type: "common", value: "c" }
    ]);
  }
});
