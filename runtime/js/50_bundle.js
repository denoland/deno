// Copyright 2018-2025 the Deno authors. MIT license.
import { core, primordials } from "ext:core/mod.js";
import { op_bundle } from "ext:core/ops";

export function bundle(options) {
  return op_bundle(options);
}