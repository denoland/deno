// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../../../testing/asserts.ts";
import { generate, validate } from "../../v1.ts";

Deno.test({
  name: "[UUID] is_valid_uuid_v1",
  fn(): void {
    const u = generate();
    const t = "63655efa-7ee6-11ea-bc55-0242ac130003";
    const n = "63655efa-7ee6-11eg-bc55-0242ac130003";

    assert(validate(u as string), `generated ${u} should be valid`);
    assert(validate(t), `${t} should be valid`);
    assert(!validate(n), `${n} should not be valid`);
  },
});
