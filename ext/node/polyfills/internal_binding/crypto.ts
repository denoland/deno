// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

export { timingSafeEqual } from "ext:deno_node/internal_binding/_timingSafeEqual.ts";
import { primordials } from "ext:core/mod.js";

const { Error } = primordials;

export function getFipsCrypto(): boolean {
  return false;
}

export function setFipsCrypto(_fips: boolean) {
  throw new Error("FIPS mode is not supported in Deno.");
}
