// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import vm from "node:vm";
import { stripColor } from "../../../test_util/std/fmt/colors.ts";
import { assertStringIncludes } from "../../../test_util/std/assert/mod.ts";

Deno.test(function inspectCrossRealmObjects() {
  assertStringIncludes(
    stripColor(
      Deno.inspect(vm.runInNewContext(`new Error("This is an error")`)),
    ),
    "Error: This is an error",
  );
  assertStringIncludes(
    stripColor(
      Deno.inspect(
        vm.runInNewContext(`new AggregateError([], "This is an error")`),
      ),
    ),
    "AggregateError: This is an error",
  );
  assertStringIncludes(
    stripColor(
      Deno.inspect(vm.runInNewContext(`new Date("2018-12-10T02:26:59.002Z")`)),
    ),
    "2018-12-10T02:26:59.002Z",
  );
});
