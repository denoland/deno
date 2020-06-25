// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../../../testing/asserts.ts";
import { generate, validate } from "../../v5.ts";

Deno.test({
  name: "[UUID] is_valid_uuid_v5",
  fn(): void {
    const u = generate({
      value: "Hello, World",
      namespace: "1b671a64-40d5-491e-99b0-da01ff1f3341",
    }) as string;
    const t = "4b4f2adc-5b27-57b5-8e3a-c4c4bcf94f05";
    const n = "4b4f2adc-5b27-17b5-8e3a-c4c4bcf94f05";

    assert(validate(u), `generated ${u} should be valid`);
    assert(validate(t), `${t} should be valid`);
    assert(!validate(n), `${n} should not be valid`);
  },
});
