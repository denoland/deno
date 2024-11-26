// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import vm from "node:vm";
import { stripAnsiCode } from "@std/fmt/colors";
import { assertStringIncludes } from "@std/assert";

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
