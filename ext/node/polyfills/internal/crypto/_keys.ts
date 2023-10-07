// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { kKeyObject } from "ext:deno_node/internal/crypto/constants.ts";

export const kKeyType = Symbol("kKeyType");

export function isKeyObject(obj: unknown): boolean {
  return (
    obj != null && (obj as Record<symbol, unknown>)[kKeyType] !== undefined
  );
}

export function isCryptoKey(obj: unknown): boolean {
  return (
    obj != null && (obj as Record<symbol, unknown>)[kKeyObject] !== undefined
  );
}
