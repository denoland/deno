// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, createResolvable } from "./test_util.ts";

unitTest({ perms: { hrtime: false } }, async function performanceNow(): Promise<
  void
> {
  const resolvable = createResolvable();
  const start = performance.now();
  setTimeout((): void => {
    const end = performance.now();
    assert(end - start >= 10);
    resolvable.resolve();
  }, 10);
  await resolvable;
});
