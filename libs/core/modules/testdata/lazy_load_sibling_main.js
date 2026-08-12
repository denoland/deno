// Copyright 2018-2026 the Deno authors. MIT license.

// Statically import both lazy-loaded ESM siblings.
import { value as aValue } from "custom:lazy_a";
import { value as bValue } from "custom:lazy_b";

if (aValue !== "b-value") {
  throw new Error("expected aValue to be 'b-value', got: " + aValue);
}
if (bValue !== "b-value") {
  throw new Error("expected bValue to be 'b-value', got: " + bValue);
}
