import { assertEquals } from "./asserts.ts";
import { stringify } from "./stringify.ts";

Deno.test("array(1)", () => {
  assertEquals(
    stringify([1, 2, 3]),
    `[
  1,
  2,
  3,
]`
  );
});

Deno.test("object(1)", () => {
  assertEquals(
    stringify({ x: { y: { z: 3 } } }),
    `{
  "x": {
    "y": {
      "z": 3,
    },
  },
}`
  );
});

Deno.test("array of objects", () => {
  assertEquals(
    stringify([{ x: 1 }, { y: 2 }, { z: 3 }]),
    `[
  {
    "x": 1,
  },
  {
    "y": 2,
  },
  {
    "z": 3,
  },
]`
  );
});

Deno.test("primitive values", () => {
  assertEquals(
    stringify([
      1,
      "hi",
      true,
      false,
      undefined,
      null,
      Symbol.for("x"),
      (x: number): number => x + 1,
      Number.call,
    ]),
    `[
  1,
  "hi",
  true,
  false,
  undefined,
  null,
  Symbol(x),
  (x) => x + 1,
  function call() { [native code] },
]`
  );
});
