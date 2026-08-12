// Copyright 2018-2026 the Deno authors. MIT license.

import vm from "node:vm";
import { stripAnsiCode } from "@std/fmt/colors";
import {
  assert,
  assertEquals,
  assertFalse,
  assertStringIncludes,
} from "@std/assert";

import { Console } from "node:console";
import process from "node:process";

Deno.test(function inspectCrossRealmObjects() {
  assertStringIncludes(
    stripAnsiCode(
      Deno.inspect(vm.runInNewContext(`new Error("This is an error")`)),
    ),
    "Error: This is an error",
  );
  assertStringIncludes(
    stripAnsiCode(
      Deno.inspect(
        vm.runInNewContext(`new AggregateError([], "This is an error")`),
      ),
    ),
    "AggregateError: This is an error",
  );
  assertStringIncludes(
    stripAnsiCode(
      Deno.inspect(vm.runInNewContext(`new Date("2018-12-10T02:26:59.002Z")`)),
    ),
    "2018-12-10T02:26:59.002Z",
  );
});

Deno.test("Console time and count methods don't throw when called with missing labels", () => {
  const console = new Console({
    stdout: process.stdout,
    stderr: process.stderr,
  });
  console.timeEnd();
  console.timeLog();
  console.time();
  console.countReset();
});

Deno.test("global console exposes lazy Node stdio streams", () => {
  const globalConsole = console as typeof console & {
    _stdout: typeof process.stdout;
    _stderr: typeof process.stderr;
  };

  assertEquals(globalConsole._stdout, process.stdout);
  assertEquals(globalConsole._stderr, process.stderr);

  const stdoutDescriptor = Object.getOwnPropertyDescriptor(
    globalConsole,
    "_stdout",
  );
  const stderrDescriptor = Object.getOwnPropertyDescriptor(
    globalConsole,
    "_stderr",
  );
  assert(stdoutDescriptor);
  assert(stderrDescriptor);
  assertFalse(stdoutDescriptor.enumerable);
  assertFalse(stderrDescriptor.enumerable);
  assertFalse(Object.keys(globalConsole).includes("_stdout"));
  assertFalse(Object.keys(globalConsole).includes("_stderr"));
});
