// Copyright 2018-2025 the Deno authors. MIT license.

// Lazily initializes the error classes in this object.
// This trick is necessary for avoiding circular dendencies between
// `internal/errors` and other modules.
// deno-lint-ignore no-explicit-any
export const codes: Record<string, any> = {};
