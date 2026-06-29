// Copyright 2018-2026 the Deno authors. MIT license.
import { createContext, runInContext } from "node:vm";
import { strictEqual } from "node:assert/strict";

const gc = (globalThis as { gc?: () => void }).gc;
strictEqual(typeof gc, "function");

function collect() {
  for (let i = 0; i < 10; i++) {
    gc!();
    const values = [];
    for (let j = 0; j < 100_000; j++) {
      values.push(j);
    }
    strictEqual(values.length, 100_000);
  }
}

// A contextified sandbox is the vulnerable path: the native ContextifyContext
// wrapper is anchored only on the sandbox object, while a function extracted
// from the context keeps the v8::Context alive. Dropping every JS reference to
// the sandbox and forcing GC must not free the wrapper out from under the
// still-live context, otherwise the global proxy interceptor would dereference
// freed memory (use-after-free).
{
  let sandbox: Record<string, unknown> | null = { foo: 42 };
  createContext(sandbox);
  const read = runInContext("() => foo", sandbox) as () => number;
  sandbox = null;

  collect();

  strictEqual(read(), 42);
}

// The same scenario, also writing back through the interceptor after GC to make
// sure both directions still reach the live sandbox.
{
  let sandbox: Record<string, unknown> | null = { value: 1 };
  createContext(sandbox);
  const read = runInContext("() => value", sandbox) as () => number;
  const write = runInContext("(v) => { value = v; }", sandbox) as (
    v: number,
  ) => void;
  sandbox = null;

  collect();

  write(123);
  strictEqual(read(), 123);
}
