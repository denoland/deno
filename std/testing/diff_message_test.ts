import { assertEquals } from "./asserts.ts";
import { diffMessageBody } from "./diff_message.ts";
import { stripColor } from "../fmt/colors.ts";

Deno.test("object with short keys", () => {
  const actual = diffMessageBody(
    { x: { y: { z: 2 } } },
    { x: { y: { z: 3 } } }
  );
  const expected = `    {
      x: {
        y: {
-         z: 2,
+         z: 3,
        },
      },
    }`;
  assertEquals(stripColor(actual), expected);
});

Deno.test("similar values but with different key order", () => {
  const actual = diffMessageBody({ a: 1, b: 2, c: 3 }, { c: 2, b: 2, a: 1 });
  const expected = `    {
      a: 1,
      b: 2,
-     c: 3,
+     c: 2,
    }`;
  assertEquals(stripColor(actual), expected);
});

Deno.test("array of objects (1)", () => {
  const actual = diffMessageBody(
    [2, { a: 1, b: 2, c: 3 }],
    [{ c: 3, b: 2, a: 1 }]
  );
  const expected = `    [
-     2,
      {
        a: 1,
        b: 2,
        c: 3,
      },
    ]`;
  assertEquals(stripColor(actual), expected);
});

Deno.test("array of objects (2)", () => {
  const actual = diffMessageBody(
    [{ a: 1, b: 2, c: 3 }],
    [{ c: 2, b: 2, a: 1 }, 2]
  );
  const expected = `    [
      {
        a: 1,
        b: 2,
+       c: 2,
-       c: 3,
      },
+     2,
    ]`;
  assertEquals(stripColor(actual), expected);
});

Deno.test("nested array objects", () => {
  const actual = diffMessageBody([{ x: [{ y: 2 }] }], [{ x: [{ y: 3 }] }]);
  const expected = `    [
      {
        x: [
          {
-           y: 2,
+           y: 3,
          },
        ],
      },
    ]`;
  assertEquals(stripColor(actual), expected);
});
