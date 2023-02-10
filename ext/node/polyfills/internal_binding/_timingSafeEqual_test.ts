// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert } from "../../testing/asserts.ts";
import { timingSafeEqual } from "./_timingSafeEqual.ts";
import { Buffer } from "../buffer.ts";

Deno.test("timingSafeEqual accepts different types of data", () => {
  const buf1 = Buffer.from("bar");
  const buf2 = Buffer.from("bar");

  assert(timingSafeEqual(buf1, buf2));
  assert(timingSafeEqual(buf1.buffer, buf2.buffer));
  assert(timingSafeEqual(new DataView(buf1.buffer), new DataView(buf2.buffer)));
});

Deno.test("timingSafeEqual works as intended", () => {
  assert(timingSafeEqual(Buffer.from("foo"), Buffer.from("foo")));
  assert(
    timingSafeEqual(Buffer.from("a"), Buffer.from("aaaaaaaaaa")) === false,
  );
});
