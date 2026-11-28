// Copyright 2018-2026 the Deno authors. MIT license.

const iterations = new Int32Array(new SharedArrayBuffer(4));
postMessage(iterations);
while (true) {
  const iteration = Atomics.add(iterations, 0, 1);
  if (iteration === 0) {
    Atomics.notify(iterations, 0);
  }
  Date.now();
}
