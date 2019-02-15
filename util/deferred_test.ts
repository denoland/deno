// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { assert, test } from "../testing/mod.ts";
import { defer, isDeferred } from "./deferred.ts";

test(async function asyncIsDeferred() {
  const d = defer();
  assert.assert(isDeferred(d));
  assert.assert(
    isDeferred({
      promise: null,
      resolve: () => {},
      reject: () => {}
    }) === false
  );
});
