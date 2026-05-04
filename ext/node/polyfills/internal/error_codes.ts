// Copyright 2018-2026 the Deno authors. MIT license.
// deno-fmt-ignore-file

// Lazily initializes the error classes in this object.
// This trick is necessary for avoiding circular dendencies between
// `internal/errors` and other modules.
(function () {
// deno-lint-ignore no-explicit-any
const codes: Record<string, any> = {};

return { codes };
})()
