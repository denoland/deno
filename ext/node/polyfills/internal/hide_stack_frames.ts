// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const { ObjectDefineProperty } = primordials;

// deno-lint-ignore no-explicit-any
type GenericFunction = (...args: any[]) => any;

/** This function removes unnecessary frames from Node.js core errors. */
export function hideStackFrames<T extends GenericFunction = GenericFunction>(
  fn: T,
): T {
  // We rename the functions that will be hidden to cut off the stacktrace
  // at the outermost one.
  const hidden = "__node_internal_" + fn.name;
  ObjectDefineProperty(fn, "name", { __proto__: null, value: hidden });

  return fn;
}
