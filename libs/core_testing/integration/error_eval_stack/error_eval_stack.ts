// Copyright 2018-2025 the Deno authors. MIT license.
// FAIL

function foo() {
  eval(`eval("eval('throw new Error()')")`);
}

foo();
