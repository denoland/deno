// Copyright 2018-2026 the Deno authors. MIT license.
delete globalThis.Error;

const e = new TypeError("e");
e.stack;
