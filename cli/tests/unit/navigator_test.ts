// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, unitTest } from "./test_util.ts";

unitTest(function navigatorNumCpus() {
  assert(navigator.hardwareConcurrency > 0);
});
