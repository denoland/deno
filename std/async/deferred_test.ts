// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { deferred } from "./deferred.ts";

Deno.test("[async] deferred", function (): Promise<void> {
  const d = deferred<number>();
  d.resolve(12);
  return Promise.resolve();
});
