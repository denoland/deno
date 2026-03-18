// Copyright 2018-2026 the Deno authors. MIT license.

// This file is here because to break a circular dependency between streams and
// crypto.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { kKeyObject } from "ext:deno_node/internal/crypto/constants.ts";
import type { KeyObject } from "ext:deno_node/internal/crypto/keys.ts";
import type { CryptoKey } from "ext:deno_crypto/00_crypto.js";

export const kKeyType = Symbol("kKeyType");

export function isKeyObject(obj: unknown): obj is KeyObject {
  return (
    obj != null && (obj as Record<symbol, unknown>)[kKeyType] !== undefined
  );
}

export function isCryptoKey(
  obj: unknown,
): obj is CryptoKey {
  return (
    obj != null && (obj as Record<symbol, unknown>)[kKeyObject] !== undefined
  );
}
