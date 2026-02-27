// Copyright 2018-2025 the Deno authors. MIT license.
delete globalThis.Error;

const e = new TypeError("e");
e.stack;
