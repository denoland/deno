// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals, assertMatch } from "./test_util.ts";

Deno.test(function errorStackMessageLine() {
  const e1 = new Error();
  e1.name = "Foo";
  e1.message = "bar";
  assertMatch(e1.stack!, /^Foo: bar\n/);

  const e2 = new Error();
  e2.name = "";
  e2.message = "bar";
  assertMatch(e2.stack!, /^bar\n/);

  const e3 = new Error();
  e3.name = "Foo";
  e3.message = "";
  assertMatch(e3.stack!, /^Foo\n/);

  const e4 = new Error();
  e4.name = "";
  e4.message = "";
  assertMatch(e4.stack!, /^\n/);

  const e5 = new Error();
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  e5.name = undefined;
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  e5.message = undefined;
  assertMatch(e5.stack!, /^Error\n/);

  const e6 = new Error();
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  e6.name = null;
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  e6.message = null;
  assertMatch(e6.stack!, /^null: null\n/);
});

Deno.test(function captureStackTrace() {
  function foo() {
    const error = new Error();
    const stack1 = error.stack!;
    Error.captureStackTrace(error, foo);
    const stack2 = error.stack!;
    // stack2 should be stack1 without the first frame.
    assertEquals(stack2, stack1.replace(/(?<=^[^\n]*\n)[^\n]*\n/, ""));
  }
  foo();
});
