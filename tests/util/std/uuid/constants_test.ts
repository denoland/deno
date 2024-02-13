// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import {
  NAMESPACE_DNS,
  NAMESPACE_OID,
  NAMESPACE_URL,
  NAMESPACE_X500,
} from "./constants.ts";
import { validate } from "./mod.ts";

Deno.test("[UUID] validate_namespaces", () => {
  assertEquals(validate(NAMESPACE_DNS), true);
  assertEquals(validate(NAMESPACE_URL), true);
  assertEquals(validate(NAMESPACE_OID), true);
  assertEquals(validate(NAMESPACE_X500), true);
});
