// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { primordials } = __bootstrap;
const { ObjectDefineProperty } = primordials;

// deno-lint-ignore no-explicit-any
type GenericFunction = (...args: any[]) => any;

/** This function removes unnecessary frames from Node.js core errors. */
function hideStackFrames<T extends GenericFunction = GenericFunction>(
  fn: T,
): T {
  // Match modern Node's `lib/internal/errors.js`: rename the function to
  // `__node_internal_<name>` and rely on stack preparation to elide frames
  // with that prefix (deno_core's `format_stack_trace` for `err.stack` and
  // `runtime/fmt_errors.rs` for runtime error display). No wrapper function:
  // these are the hottest validators in the node compat layer, and a wrapper
  // costs a rest-args array + try/catch on every call.
  const hidden = "__node_internal_" + fn.name;
  ObjectDefineProperty(fn, "name", { __proto__: null, value: hidden });
  return fn;
}

return { hideStackFrames };
})();
