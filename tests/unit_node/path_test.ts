// Copyright 2018-2025 the Deno authors. MIT license.

import path from "node:path";
import posix from "node:path/posix";
import win32 from "node:path/win32";

import { assertStrictEquals } from "@std/assert";

Deno.test("[node/path] posix and win32 objects", () => {
  assertStrictEquals(path.posix, posix);
  assertStrictEquals(path.win32, win32);
  assertStrictEquals(path.posix, path.posix.posix);
  assertStrictEquals(path.win32, path.posix.win32);
  assertStrictEquals(path.posix, path.win32.posix);
  assertStrictEquals(path.win32, path.win32.win32);
});
