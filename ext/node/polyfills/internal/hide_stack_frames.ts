// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { primordials } = globalThis.__bootstrap;
const { ErrorCaptureStackTrace, ObjectDefineProperty, ReflectApply } =
  primordials;

// deno-lint-ignore no-explicit-any
type GenericFunction = (...args: any[]) => any;

/** This function removes unnecessary frames from Node.js core errors. */
function hideStackFrames<T extends GenericFunction = GenericFunction>(
  fn: T,
): T {
  // Match Node's `lib/internal/errors.js`: wrap so a thrown error has its stack
  // re-captured at the call site via Error.captureStackTrace. For callers that
  // RETURN an Error rather than throwing, the V8-captured stack still includes
  // the wrapper and the inner function -- so also rename both to
  // `__node_internal_<name>`, which Deno's stack formatter
  // (`runtime/fmt_errors.rs`) elides from user-visible output.
  const hidden = "__node_internal_" + fn.name;
  // deno-lint-ignore no-explicit-any
  function wrappedFn(this: any, ...args: any[]) {
    try {
      return ReflectApply(fn, this, args);
    } catch (error) {
      // deno-lint-ignore prefer-primordials
      if (Error.stackTraceLimit) {
        ErrorCaptureStackTrace(error, wrappedFn);
      }
      throw error;
    }
  }
  ObjectDefineProperty(wrappedFn, "name", { __proto__: null, value: hidden });
  ObjectDefineProperty(fn, "name", { __proto__: null, value: hidden });
  // deno-lint-ignore no-explicit-any
  (wrappedFn as any).withoutStackTrace = fn;
  return wrappedFn as unknown as T;
}

return { hideStackFrames };
})();
