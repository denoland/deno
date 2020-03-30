// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../../../testing/asserts.ts";
import { generate, validate } from "../../v4.ts";

Deno.test({
  name: "[UUID] is_valid_uuid_v4",
  fn(): void {
    const u = generate();
    const t = "84fb7824-b951-490e-8afd-0c13228a8282";
    const n = "84fb7824-b951-490g-8afd-0c13228a8282";

    assert(validate(u), `generated ${u} should be valid`);
    assert(validate(t), `${t} should be valid`);
    assert(!validate(n), `${n} should not be valid`);
  },
});
