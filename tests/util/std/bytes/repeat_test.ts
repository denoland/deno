// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows } from "../assert/mod.ts";
import { repeat } from "./repeat.ts";

Deno.test("[bytes] repeat", () => {
  // input / output / count / error message
  const repeatTestCase = [
    ["", "", 0],
    ["", "", 1],
    ["", "", 1.1, "bytes: repeat count must be an integer"],
    ["", "", 2],
    ["", "", 0],
    ["-", "", 0],
    ["-", "-", -1, "bytes: negative repeat count"],
    ["-", "----------", 10],
    ["abc ", "abc abc abc ", 3],
  ];
  for (const [input, output, count, errMsg] of repeatTestCase) {
    if (errMsg) {
      assertThrows(
        () => {
          repeat(new TextEncoder().encode(input as string), count as number);
        },
        Error,
        errMsg as string,
      );
    } else {
      const newBytes = repeat(
        new TextEncoder().encode(input as string),
        count as number,
      );

      assertEquals(new TextDecoder().decode(newBytes), output);
    }
  }
});
