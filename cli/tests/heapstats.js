// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

function allocTest(alloc, allocAssert, deallocAssert) {
  // Helper func that GCs then returns memory usage
  const sample = () => {
    // deno-lint-ignore no-undef
    gc();
    return Deno.memoryUsage();
  };
  const delta = (t1, t2) => t2.heapUsed - t1.heapUsed;

  // Sample "clean" heap usage
  const t1 = sample();

  // Alloc
  let x = alloc();
  const t2 = sample();
  allocAssert(delta(t1, t2));

  // Free
  x = null;
  const t3 = sample();
  deallocAssert(delta(t2, t3));
}

function main() {
  // Large-array test, 1M slot array consumes ~4MB (4B per slot)
  allocTest(
    () => new Array(1e6),
    (delta) => console.log("Allocated:", Math.round(delta / 1e6) + "MB"),
    (delta) => console.log("Freed:", Math.round(delta / 1e6) + "MB"),
  );
}

main();
