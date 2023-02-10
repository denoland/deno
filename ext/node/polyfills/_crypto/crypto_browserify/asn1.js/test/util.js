// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Fedor Indutny. All rights reserved. MIT license.

import { assertEquals } from "SOMETHING IS BROKEN HERE ../../../../../testing/asserts.ts";

export function jsonEqual(a, b) {
  assertEquals(JSON.parse(JSON.stringify(a)), JSON.parse(JSON.stringify(b)));
}
