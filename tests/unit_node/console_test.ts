// Copyright 2018-2025 the Deno authors. MIT license.

import vm from "node:vm";
import { stripAnsiCode } from "@std/fmt/colors";
import { assertStringIncludes } from "@std/assert";

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
