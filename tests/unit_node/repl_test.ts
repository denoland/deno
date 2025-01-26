// deno-lint-ignore-file no-undef
// Copyright 2018-2025 the Deno authors. MIT license.

import repl from "node:repl";
import { assert } from "@std/assert";

Deno.test({
  name: "repl module exports",
  fn() {
    assert(typeof repl.REPLServer !== "undefined");
    assert(typeof repl.start !== "undefined");
    // @ts-ignore not present in declaration files, but libraries depend on it
    assert(typeof repl.builtinModules !== "undefined");
    // @ts-ignore not present in declaration files, but libraries depend on it
    assert(typeof repl._builtinLibs !== "undefined");
  },
});
