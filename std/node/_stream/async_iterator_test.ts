// Copyright Node.js contributors. All rights reserved. MIT License.
import { deferred } from "../../async/mod.ts";
import { assertEquals, assertThrowsAsync } from "../../testing/asserts.ts";
import toReadableAsyncIterator from "./async_iterator.ts";
import Readable from "./readable.ts";
import Stream from "./stream.ts";

Deno.test("Stream to async iterator", async () => {
  let destroyExecuted = 0;
  const destroyExecutedExpected = 1;
  const destroyExpectedExecutions = deferred();

  class AsyncIteratorStream extends Stream {
    constructor() {
      super();
    }

    destroy() {
      destroyExecuted++;
      if (destroyExecuted == destroyExecutedExpected) {
        destroyExpectedExecutions.resolve();
      }
    }

    [Symbol.asyncIterator] = Readable.prototype[Symbol.asyncIterator];
  }

  const stream = new AsyncIteratorStream();

  queueMicrotask(() => {
    stream.emit("data", "hello");
    stream.emit("data", "world");
    stream.emit("end");
  });

  let res = "";

  for await (const d of stream) {
    res += d;
  }
  assertEquals(res, "helloworld");

  const destroyTimeout = setTimeout(
    () => destroyExpectedExecutions.reject(),
    1000,
  );
  await destroyExpectedExecutions;
  clearTimeout(destroyTimeout);
  assertEquals(destroyExecuted, destroyExecutedExpected);
});

Deno.test("Stream to async iterator throws on 'error' emitted", async () => {
  let closeExecuted = 0;
  const closeExecutedExpected = 1;
  const closeExpectedExecutions = deferred();

  let errorExecuted = 0;
  const errorExecutedExpected = 1;
  const errorExpectedExecutions = deferred();

  class StreamImplementation extends Stream {
    close() {
      closeExecuted++;
      if (closeExecuted == closeExecutedExpected) {
        closeExpectedExecutions.resolve();
      }
    }
  }

  const stream = new StreamImplementation();
  queueMicrotask(() => {
    stream.emit("data", 0);
    stream.emit("data", 1);
    stream.emit("error", new Error("asd"));
  });

  toReadableAsyncIterator(stream)
    .next()
    .catch((err) => {
      errorExecuted++;
      if (errorExecuted == errorExecutedExpected) {
        errorExpectedExecutions.resolve();
      }
      assertEquals(err.message, "asd");
    });

  const closeTimeout = setTimeout(
    () => closeExpectedExecutions.reject(),
    1000,
  );
  const errorTimeout = setTimeout(
    () => errorExpectedExecutions.reject(),
    1000,
  );
  await closeExpectedExecutions;
  await errorExpectedExecutions;
  clearTimeout(closeTimeout);
  clearTimeout(errorTimeout);
  assertEquals(closeExecuted, closeExecutedExpected);
  assertEquals(errorExecuted, errorExecutedExpected);
});

Deno.test("Async iterator matches values of Readable", async () => {
  const readable = new Readable({
    objectMode: true,
    read() {},
  });
  readable.push(0);
  readable.push(1);
  readable.push(null);

  const iter = readable[Symbol.asyncIterator]();

  assertEquals(
    await iter.next().then(({ value }) => value),
    0,
  );
  for await (const d of iter) {
    assertEquals(d, 1);
  }
});

Deno.test("Async iterator throws on Readable destroyed sync", async () => {
  const message = "kaboom from read";

  const readable = new Readable({
    objectMode: true,
    read() {
      this.destroy(new Error(message));
    },
  });

  await assertThrowsAsync(
    async () => {
      // deno-lint-ignore no-empty
      for await (const k of readable) {}
    },
    Error,
    message,
  );
});

Deno.test("Async iterator throws on Readable destroyed async", async () => {
  const message = "kaboom";
  const readable = new Readable({
    read() {},
  });
  const iterator = readable[Symbol.asyncIterator]();

  readable.destroy(new Error(message));

  await assertThrowsAsync(
    iterator.next.bind(iterator),
    Error,
    message,
  );
});

Deno.test("Async iterator finishes the iterator when Readable destroyed", async () => {
  const readable = new Readable({
    read() {},
  });

  readable.destroy();

  const { done } = await readable[Symbol.asyncIterator]().next();
  assertEquals(done, true);
});

Deno.test("Async iterator finishes all item promises when Readable destroyed", async () => {
  const r = new Readable({
    objectMode: true,
    read() {
    },
  });

  const b = r[Symbol.asyncIterator]();
  const c = b.next();
  const d = b.next();
  r.destroy();
  assertEquals(await c, { done: true, value: undefined });
  assertEquals(await d, { done: true, value: undefined });
});

Deno.test("Async iterator: 'next' is triggered by Readable push", async () => {
  const max = 42;
  let readed = 0;
  let received = 0;
  const readable = new Readable({
    objectMode: true,
    read() {
      this.push("hello");
      if (++readed === max) {
        this.push(null);
      }
    },
  });

  for await (const k of readable) {
    received++;
    assertEquals(k, "hello");
  }

  assertEquals(readed, received);
});

Deno.test("Async iterator: 'close' called on forced iteration end", async () => {
  let closeExecuted = 0;
  const closeExecutedExpected = 1;
  const closeExpectedExecutions = deferred();

  class IndestructibleReadable extends Readable {
    constructor() {
      super({
        autoDestroy: false,
        read() {},
      });
    }

    close() {
      closeExecuted++;
      if (closeExecuted == closeExecutedExpected) {
        closeExpectedExecutions.resolve();
      }
      readable.emit("close");
    }

    // deno-lint-ignore ban-ts-comment
    //@ts-ignore
    destroy = null;
  }

  const readable = new IndestructibleReadable();
  readable.push("asd");
  readable.push("asd");

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  for await (const d of readable) {
    break;
  }

  const closeTimeout = setTimeout(
    () => closeExpectedExecutions.reject(),
    1000,
  );
  await closeExpectedExecutions;
  clearTimeout(closeTimeout);
  assertEquals(closeExecuted, closeExecutedExpected);
});
