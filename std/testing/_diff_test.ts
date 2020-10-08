// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { diff } from "./_diff.ts";
import { assertEquals } from "../testing/asserts.ts";

Deno.test({
  name: "empty",
  fn(): void {
    assertEquals(diff([], []), []);
  },
});

Deno.test({
  name: '"a" vs "b"',
  fn(): void {
    assertEquals(diff(["a"], ["b"]), [
      { type: "removed", value: "a" },
      { type: "added", value: "b" },
    ]);
  },
});

Deno.test({
  name: '"a" vs "a"',
  fn(): void {
    assertEquals(diff(["a"], ["a"]), [{ type: "common", value: "a" }]);
  },
});

Deno.test({
  name: '"a" vs ""',
  fn(): void {
    assertEquals(diff(["a"], []), [{ type: "removed", value: "a" }]);
  },
});

Deno.test({
  name: '"" vs "a"',
  fn(): void {
    assertEquals(diff([], ["a"]), [{ type: "added", value: "a" }]);
  },
});

Deno.test({
  name: '"a" vs "a, b"',
  fn(): void {
    assertEquals(diff(["a"], ["a", "b"]), [
      { type: "common", value: "a" },
      { type: "added", value: "b" },
    ]);
  },
});

Deno.test({
  name: '"strength" vs "string"',
  fn(): void {
    assertEquals(diff(Array.from("strength"), Array.from("string")), [
      { type: "common", value: "s" },
      { type: "common", value: "t" },
      { type: "common", value: "r" },
      { type: "removed", value: "e" },
      { type: "added", value: "i" },
      { type: "common", value: "n" },
      { type: "common", value: "g" },
      { type: "removed", value: "t" },
      { type: "removed", value: "h" },
    ]);
  },
});

Deno.test({
  name: '"strength" vs ""',
  fn(): void {
    assertEquals(diff(Array.from("strength"), Array.from("")), [
      { type: "removed", value: "s" },
      { type: "removed", value: "t" },
      { type: "removed", value: "r" },
      { type: "removed", value: "e" },
      { type: "removed", value: "n" },
      { type: "removed", value: "g" },
      { type: "removed", value: "t" },
      { type: "removed", value: "h" },
    ]);
  },
});

Deno.test({
  name: '"" vs "strength"',
  fn(): void {
    assertEquals(diff(Array.from(""), Array.from("strength")), [
      { type: "added", value: "s" },
      { type: "added", value: "t" },
      { type: "added", value: "r" },
      { type: "added", value: "e" },
      { type: "added", value: "n" },
      { type: "added", value: "g" },
      { type: "added", value: "t" },
      { type: "added", value: "h" },
    ]);
  },
});

Deno.test({
  name: '"abc", "c" vs "abc", "bcd", "c"',
  fn(): void {
    assertEquals(diff(["abc", "c"], ["abc", "bcd", "c"]), [
      { type: "common", value: "abc" },
      { type: "added", value: "bcd" },
      { type: "common", value: "c" },
    ]);
  },
});
