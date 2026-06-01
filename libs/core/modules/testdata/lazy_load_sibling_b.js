// Copyright 2018-2026 the Deno authors. MIT license.

// `value` is in the temporal dead zone until this module is evaluated.
// If A retrieves B's namespace before B has evaluated, accessing `.value`
// throws `Cannot access 'value' before initialization`.
export const value = "b-value";
