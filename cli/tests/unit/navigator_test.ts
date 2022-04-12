// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assert } from "./test_util.ts";

Deno.test(function navigatorNumCpus() {
  assert(navigator.hardwareConcurrency > 0);
});
