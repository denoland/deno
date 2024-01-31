// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { core, primordials } from "ext:core/mod.js";
import { serializePermissions } from "ext:runtime/10_permissions.js";
const ops = core.ops;
const {
  StringPrototypeReplaceAll,
  SafeArrayIterator,
} = primordials;

const ESCAPE_ASCII_CHARS = [
  ["\b", "\\b"],
  ["\f", "\\f"],
  ["\t", "\\t"],
  ["\n", "\\n"],
  ["\r", "\\r"],
  ["\v", "\\v"],
];

/**
 * @param {string} name
 * @returns {string}
 */
export function escapeName(name) {
  // Check if we need to escape a character
  for (let i = 0; i < name.length; i++) {
    const ch = name.charCodeAt(i);
    if (ch <= 13 && ch >= 8) {
      // Slow path: We do need to escape it
      for (const [escape, replaceWith] of ESCAPE_ASCII_CHARS) {
        name = StringPrototypeReplaceAll(name, escape, replaceWith);
      }
      return name;
    }
  }

  // We didn't need to escape anything, return original string
  return name;
}

export function pledgePermissions(permissions) {
  return ops.op_pledge_test_permissions(
    serializePermissions(permissions),
  );
}

export function restorePermissions(token) {
  ops.op_restore_test_permissions(token);
}

export function withPermissions(fn, permissions) {
  return async function applyPermissions(...params) {
    const token = pledgePermissions(permissions);

    try {
      return await fn(...new SafeArrayIterator(params));
    } finally {
      restorePermissions(token);
    }
  };
}
