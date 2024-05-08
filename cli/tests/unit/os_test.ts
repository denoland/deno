// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrows } from "../../testing/asserts.ts";

Deno.test("Deno.exitCode getter and setter", () => {
  // Initial value is 0
  assertEquals(Deno.exitCode, 0);

  // Set a new value
  Deno.exitCode = 5;
  assertEquals(Deno.exitCode, 5);

  // Reset to initial value
  Deno.exitCode = 0;
  assertEquals(Deno.exitCode, 0);
});

Deno.test("Setting Deno.exitCode to NaN throws TypeError", () => {
  // @ts-expect-error;
  Deno.exitCode = "123";
  assertEquals(Deno.exitCode, 123);

  // Reset
  Deno.exitCode = 0;
  assertEquals(Deno.exitCode, 0);

  // Throws on non-number values
  assertThrows(
    () => {
      // @ts-expect-error Testing for runtime error
      Deno.exitCode = "not a number";
    },
    TypeError,
    "Exit code must be a number.",
  );
});

Deno.test("Setting Deno.exitCode does not cause an immediate exit", () => {
  let exited = false;
  const originalExit = Deno.exit;

  // @ts-expect-error; read-only
  Deno.exit = () => {
    exited = true;
  };

  Deno.exitCode = 1;
  assertEquals(exited, false);

  // @ts-expect-error; read-only
  Deno.exit = originalExit;
});

Deno.test("Running Deno.exit(value) overrides Deno.exitCode", () => {
  let args: unknown[] | undefined;

  const originalExit = Deno.exit;
  // @ts-expect-error; read-only
  Deno.exit = (...x) => {
    args = x;
  };

  Deno.exitCode = 42;
  Deno.exit(0);

  assertEquals(args, [0]);
  // @ts-expect-error; read-only
  Deno.exit = originalExit;
});

Deno.test("Running Deno.exit() uses Deno.exitCode as fallback", () => {
  let args: unknown[] | undefined;

  const originalExit = Deno.exit;
  // @ts-expect-error; read-only
  Deno.exit = (...x) => {
    args = x;
  };

  Deno.exitCode = 42;
  Deno.exit();

  assertEquals(args, [42]);
  // @ts-expect-error; read-only
  Deno.exit = originalExit;
});

Deno.test("Retrieving the set exit code before process termination", () => {
  Deno.exitCode = 42;
  assertEquals(Deno.exitCode, 42);

  // Reset to initial value
  Deno.exitCode = 0;
});
