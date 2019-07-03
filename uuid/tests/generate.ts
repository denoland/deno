// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "../../testing/asserts.ts";
import { test } from "../../testing/mod.ts";
import mod, { validate, v4 } from "../mod.ts";
import { validate as validate4 } from "../v4.ts";

test({
  name: "[UUID] uuid_v4",
  fn(): void {
    const u = mod();
    assertEquals(typeof u, "string", "returns a string");
    assert(u !== "", "return string is not empty");
  }
});

test({
  name: "[UUID] uuid_v4_format",
  fn(): void {
    for (let i = 0; i < 10000; i++) {
      const u = mod() as string;
      assert(validate(u), `${u} is not a valid uuid v4`);
    }
  }
});

test({
  name: "[UUID] default_is_v4",
  fn(): void {
    assertEquals(mod, v4, "default is v4");
    assertEquals(validate, validate4, "validate is v4");
  }
});
