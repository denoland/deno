// Copyright 2018-2026 the Deno authors. MIT license.
import { core, primordials } from "ext:core/mod.js";
const { serializePermissions } = core.loadExtScript(
  "ext:runtime/10_permissions.js",
);
const ops = core.ops;
const {
  NumberPrototypeToString,
  SafeArrayIterator,
  StringPrototypeCharCodeAt,
  StringPrototypePadStart,
  StringPrototypeSlice,
} = primordials;

function escapeControlChar(code) {
  switch (code) {
    case 0x08:
      return "\\b";
    case 0x09:
      return "\\t";
    case 0x0a:
      return "\\n";
    case 0x0b:
      return "\\v";
    case 0x0c:
      return "\\f";
    case 0x0d:
      return "\\r";
  }
  return `\\x${
    StringPrototypePadStart(NumberPrototypeToString(code, 16), 2, "0")
  }`;
}

/**
 * @param {string} name
 * @returns {string}
 */
export function escapeName(name) {
  let escapedName = "";
  let lastIndex = 0;

  for (let i = 0; i < name.length; i++) {
    const code = StringPrototypeCharCodeAt(name, i);
    if (code <= 0x1f || code === 0x7f || (code >= 0x80 && code <= 0x9f)) {
      escapedName += StringPrototypeSlice(name, lastIndex, i) +
        escapeControlChar(code);
      lastIndex = i + 1;
    }
  }

  if (lastIndex !== 0) {
    return escapedName + StringPrototypeSlice(name, lastIndex);
  }

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
