// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

export { timingSafeEqual } from "ext:deno_node/internal_binding/_timingSafeEqual.ts";
import { primordials } from "ext:core/mod.js";

const { Error } = primordials;

export function getFipsCrypto(): number {
  return 0;
}

export function setFipsCrypto(_fips: boolean) {
  throw new Error("fips mode not supported");
}

export function testFipsCrypto(): number {
  return 0;
}
