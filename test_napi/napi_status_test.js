// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals, loadTestLibrary } from "./common.js";

const napiStatus = loadTestLibrary();

Deno.test("status", function () {
  napiStatus.createNapiError();
  assert(napiStatus.testNapiErrorCleanup());
});
