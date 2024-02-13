// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { tee } from "./tee.ts";
import { assertEquals } from "../assert/mod.ts";

/** An example async generator */
const gen = async function* iter() {
  yield 1;
  yield 2;
  yield 3;
};

Deno.test("async/tee - 2 branches", async () => {
  const iter = gen();
  const [res0, res1] = tee(iter).map(async (src) => await Array.fromAsync(src));
  assertEquals(
    await Promise.all([res0, res1]),
    [
      [1, 2, 3],
      [1, 2, 3],
    ],
  );
});

Deno.test("async/tee - 3 branches - immediate consumption", async () => {
  const iter = gen();
  const [res0, res1, res2] = tee(iter, 3).map(async (src) =>
    await Array.fromAsync(src)
  );
  assertEquals(
    await Promise.all([res0, res1, res2]),
    [
      [1, 2, 3],
      [1, 2, 3],
      [1, 2, 3],
    ],
  );
});

Deno.test("async/tee - 3 branches - delayed consumption", async () => {
  const iter = gen();
  const iters = tee(iter, 3);

  await new Promise<void>((resolve) => {
    setTimeout(() => resolve(), 20);
  });

  assertEquals(
    await Promise.all(iters.map(async (src) => await Array.fromAsync(src))),
    [
      [1, 2, 3],
      [1, 2, 3],
      [1, 2, 3],
    ],
  );
});

Deno.test("async/tee - concurrent .next calls", async () => {
  const [left] = tee(gen());
  const l = left[Symbol.asyncIterator]();
  assertEquals(await Promise.all([l.next(), l.next(), l.next(), l.next()]), [{
    value: 1,
    done: false,
  }, {
    value: 2,
    done: false,
  }, {
    value: 3,
    done: false,
  }, {
    value: undefined,
    done: true,
  }]);
});
