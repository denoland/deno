// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals } from "./test_util.ts";

// TODO(@kitsonk) remove when we are no longer patching TypeScript to have
// these types available.

Deno.test(function typeCheckingEsNextArrayString() {
  const b = ["a", "b", "c", "d", "e", "f"];
  assertEquals(b.findLast((val) => typeof val === "string"), "f");
  assertEquals(b.findLastIndex((val) => typeof val === "string"), 5);
});

Deno.test(function intlListFormat() {
  const formatter = new Intl.ListFormat("en", {
    style: "long",
    type: "conjunction",
  });
  assertEquals(
    formatter.format(["red", "green", "blue"]),
    "red, green, and blue",
  );

  const formatter2 = new Intl.ListFormat("en", {
    style: "short",
    type: "disjunction",
  });
  assertEquals(formatter2.formatToParts(["Rust", "golang"]), [
    { type: "element", value: "Rust" },
    { type: "literal", value: " or " },
    { type: "element", value: "golang" },
  ]);

  // Works with iterables as well
  assertEquals(
    formatter.format(new Set(["red", "green", "blue"])),
    "red, green, and blue",
  );
  assertEquals(formatter2.formatToParts(new Set(["Rust", "golang"])), [
    { type: "element", value: "Rust" },
    { type: "literal", value: " or " },
    { type: "element", value: "golang" },
  ]);
});

Deno.test(function setUnion() {
  const a = new Set([1, 2, 3]);
  const b = new Set([3, 4, 5]);
  const union = a.union(b);
  assertEquals(union, new Set([1, 2, 3, 4, 5]));
});

Deno.test(function float16Array() {
  const myNums = Float16Array.from([11.25, 2, -22.5, 1]);
  const sorted = myNums.toSorted((a, b) => a - b);
  assertEquals(sorted[0], -22.5);
  assertEquals(sorted[1], 1);
  assertEquals(sorted[2], 2);
  assertEquals(sorted[3], 11.25);
});
