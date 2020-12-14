// Copyright Node.js contributors. All rights reserved. MIT License.
import { Buffer } from "../buffer.ts";
import Readable from "../_stream/readable.ts";
import { once } from "../events.ts";
import { deferred } from "../../async/mod.ts";
import {
  assert,
  assertEquals,
  assertStrictEquals,
} from "../../testing/asserts.ts";

Deno.test("Readable stream from iterator", async () => {
  function* generate() {
    yield "a";
    yield "b";
    yield "c";
  }

  const stream = Readable.from(generate());

  const expected = ["a", "b", "c"];

  for await (const chunk of stream) {
    assertStrictEquals(chunk, expected.shift());
  }
});

Deno.test("Readable stream from async iterator", async () => {
  async function* generate() {
    yield "a";
    yield "b";
    yield "c";
  }

  const stream = Readable.from(generate());

  const expected = ["a", "b", "c"];

  for await (const chunk of stream) {
    assertStrictEquals(chunk, expected.shift());
  }
});

Deno.test("Readable stream from promise", async () => {
  const promises = [
    Promise.resolve("a"),
    Promise.resolve("b"),
    Promise.resolve("c"),
  ];

  const stream = Readable.from(promises);

  const expected = ["a", "b", "c"];

  for await (const chunk of stream) {
    assertStrictEquals(chunk, expected.shift());
  }
});

Deno.test("Readable stream from string", async () => {
  const string = "abc";
  const stream = Readable.from(string);

  for await (const chunk of stream) {
    assertStrictEquals(chunk, string);
  }
});

Deno.test("Readable stream from Buffer", async () => {
  const string = "abc";
  const stream = Readable.from(Buffer.from(string));

  for await (const chunk of stream) {
    assertStrictEquals((chunk as Buffer).toString(), string);
  }
});

Deno.test("Readable stream gets destroyed on error", async () => {
  // deno-lint-ignore require-yield
  async function* generate() {
    throw new Error("kaboom");
  }

  const stream = Readable.from(generate());

  stream.read();

  const [err] = await once(stream, "error");
  assertStrictEquals(err.message, "kaboom");
  assertStrictEquals(stream.destroyed, true);
});

Deno.test("Readable stream works as Transform stream", async () => {
  async function* generate(stream: Readable) {
    for await (const chunk of stream) {
      yield (chunk as string).toUpperCase();
    }
  }

  const source = new Readable({
    objectMode: true,
    read() {
      this.push("a");
      this.push("b");
      this.push("c");
      this.push(null);
    },
  });

  const stream = Readable.from(generate(source));

  const expected = ["A", "B", "C"];

  for await (const chunk of stream) {
    assertStrictEquals(chunk, expected.shift());
  }
});

Deno.test("Readable stream can be paused", () => {
  const readable = new Readable();

  // _read is a noop, here.
  readable._read = () => {};

  // Default state of a stream is not "paused"
  assert(!readable.isPaused());

  // Make the stream start flowing...
  readable.on("data", () => {});

  // still not paused.
  assert(!readable.isPaused());

  readable.pause();
  assert(readable.isPaused());
  readable.resume();
  assert(!readable.isPaused());
});

Deno.test("Readable stream sets enconding correctly", () => {
  const readable = new Readable({
    read() {},
  });

  readable.setEncoding("utf8");

  readable.push(new TextEncoder().encode("DEF"));
  readable.unshift(new TextEncoder().encode("ABC"));

  assertStrictEquals(readable.read(), "ABCDEF");
});

Deno.test("Readable stream sets encoding correctly", () => {
  const readable = new Readable({
    read() {},
  });

  readable.setEncoding("utf8");

  readable.push(new TextEncoder().encode("DEF"));
  readable.unshift(new TextEncoder().encode("ABC"));

  assertStrictEquals(readable.read(), "ABCDEF");
});

Deno.test("Readable stream holds up a big push", async () => {
  let readExecuted = 0;
  const readExecutedExpected = 3;
  const readExpectedExecutions = deferred();

  let endExecuted = 0;
  const endExecutedExpected = 1;
  const endExpectedExecutions = deferred();

  const str = "asdfasdfasdfasdfasdf";

  const r = new Readable({
    highWaterMark: 5,
    encoding: "utf8",
  });

  let reads = 0;

  function _read() {
    if (reads === 0) {
      setTimeout(() => {
        r.push(str);
      }, 1);
      reads++;
    } else if (reads === 1) {
      const ret = r.push(str);
      assertEquals(ret, false);
      reads++;
    } else {
      r.push(null);
    }
  }

  r._read = () => {
    readExecuted++;
    if (readExecuted == readExecutedExpected) {
      readExpectedExecutions.resolve();
    }
    _read();
  };

  r.on("end", () => {
    endExecuted++;
    if (endExecuted == endExecutedExpected) {
      endExpectedExecutions.resolve();
    }
  });

  // Push some data in to start.
  // We've never gotten any read event at this point.
  const ret = r.push(str);
  assert(!ret);
  let chunk = r.read();
  assertEquals(chunk, str);
  chunk = r.read();
  assertEquals(chunk, null);

  r.once("readable", () => {
    // This time, we'll get *all* the remaining data, because
    // it's been added synchronously, as the read WOULD take
    // us below the hwm, and so it triggered a _read() again,
    // which synchronously added more, which we then return.
    chunk = r.read();
    assertEquals(chunk, str + str);

    chunk = r.read();
    assertEquals(chunk, null);
  });

  const readTimeout = setTimeout(
    () => readExpectedExecutions.reject(),
    1000,
  );
  const endTimeout = setTimeout(
    () => endExpectedExecutions.reject(),
    1000,
  );
  await readExpectedExecutions;
  await endExpectedExecutions;
  clearTimeout(readTimeout);
  clearTimeout(endTimeout);
  assertEquals(readExecuted, readExecutedExpected);
  assertEquals(endExecuted, endExecutedExpected);
});

Deno.test("Readable stream: 'on' event", async () => {
  async function* generate() {
    yield "a";
    yield "b";
    yield "c";
  }

  const stream = Readable.from(generate());

  let iterations = 0;
  const expected = ["a", "b", "c"];

  stream.on("data", (chunk) => {
    iterations++;
    assertStrictEquals(chunk, expected.shift());
  });

  await once(stream, "end");

  assertStrictEquals(iterations, 3);
});

Deno.test("Readable stream: 'data' event", async () => {
  async function* generate() {
    yield "a";
    yield "b";
    yield "c";
  }

  const stream = Readable.from(generate(), { objectMode: false });

  let iterations = 0;
  const expected = ["a", "b", "c"];

  stream.on("data", (chunk) => {
    iterations++;
    assertStrictEquals(chunk instanceof Buffer, true);
    assertStrictEquals(chunk.toString(), expected.shift());
  });

  await once(stream, "end");

  assertStrictEquals(iterations, 3);
});

Deno.test("Readable stream: 'data' event on non-object", async () => {
  async function* generate() {
    yield "a";
    yield "b";
    yield "c";
  }

  const stream = Readable.from(generate(), { objectMode: false });

  let iterations = 0;
  const expected = ["a", "b", "c"];

  stream.on("data", (chunk) => {
    iterations++;
    assertStrictEquals(chunk instanceof Buffer, true);
    assertStrictEquals(chunk.toString(), expected.shift());
  });

  await once(stream, "end");

  assertStrictEquals(iterations, 3);
});

Deno.test("Readable stream: 'readable' event is emitted but 'read' is not on highWaterMark length exceeded", async () => {
  let readableExecuted = 0;
  const readableExecutedExpected = 1;
  const readableExpectedExecutions = deferred();

  const r = new Readable({
    highWaterMark: 3,
  });

  r._read = () => {
    throw new Error("_read must not be called");
  };
  r.push(Buffer.from("blerg"));

  setTimeout(function () {
    assert(!r._readableState.reading);
    r.on("readable", () => {
      readableExecuted++;
      if (readableExecuted == readableExecutedExpected) {
        readableExpectedExecutions.resolve();
      }
    });
  }, 1);

  const readableTimeout = setTimeout(
    () => readableExpectedExecutions.reject(),
    1000,
  );
  await readableExpectedExecutions;
  clearTimeout(readableTimeout);
  assertEquals(readableExecuted, readableExecutedExpected);
});

Deno.test("Readable stream: 'readable' and 'read' events are emitted on highWaterMark length not reached", async () => {
  let readableExecuted = 0;
  const readableExecutedExpected = 1;
  const readableExpectedExecutions = deferred();

  let readExecuted = 0;
  const readExecutedExpected = 1;
  const readExpectedExecutions = deferred();

  const r = new Readable({
    highWaterMark: 3,
  });

  r._read = () => {
    readExecuted++;
    if (readExecuted == readExecutedExpected) {
      readExpectedExecutions.resolve();
    }
  };

  r.push(Buffer.from("bl"));

  setTimeout(function () {
    assert(r._readableState.reading);
    r.on("readable", () => {
      readableExecuted++;
      if (readableExecuted == readableExecutedExpected) {
        readableExpectedExecutions.resolve();
      }
    });
  }, 1);

  const readableTimeout = setTimeout(
    () => readableExpectedExecutions.reject(),
    1000,
  );
  const readTimeout = setTimeout(
    () => readExpectedExecutions.reject(),
    1000,
  );
  await readableExpectedExecutions;
  await readExpectedExecutions;
  clearTimeout(readableTimeout);
  clearTimeout(readTimeout);
  assertEquals(readableExecuted, readableExecutedExpected);
  assertEquals(readExecuted, readExecutedExpected);
});

Deno.test("Readable stream: 'readable' event is emitted but 'read' is not on highWaterMark length not reached and stream ended", async () => {
  let readableExecuted = 0;
  const readableExecutedExpected = 1;
  const readableExpectedExecutions = deferred();

  const r = new Readable({
    highWaterMark: 30,
  });

  r._read = () => {
    throw new Error("Must not be executed");
  };

  r.push(Buffer.from("blerg"));
  //This ends the stream and triggers end
  r.push(null);

  setTimeout(function () {
    // Assert we're testing what we think we are
    assert(!r._readableState.reading);
    r.on("readable", () => {
      readableExecuted++;
      if (readableExecuted == readableExecutedExpected) {
        readableExpectedExecutions.resolve();
      }
    });
  }, 1);

  const readableTimeout = setTimeout(
    () => readableExpectedExecutions.reject(),
    1000,
  );
  await readableExpectedExecutions;
  clearTimeout(readableTimeout);
  assertEquals(readableExecuted, readableExecutedExpected);
});

Deno.test("Readable stream: 'read' is emitted on empty string pushed in non-object mode", async () => {
  let endExecuted = 0;
  const endExecutedExpected = 1;
  const endExpectedExecutions = deferred();

  const underlyingData = ["", "x", "y", "", "z"];
  const expected = underlyingData.filter((data) => data);
  const result: unknown[] = [];

  const r = new Readable({
    encoding: "utf8",
  });
  r._read = function () {
    queueMicrotask(() => {
      if (!underlyingData.length) {
        this.push(null);
      } else {
        this.push(underlyingData.shift());
      }
    });
  };

  r.on("readable", () => {
    const data = r.read();
    if (data !== null) result.push(data);
  });

  r.on("end", () => {
    endExecuted++;
    if (endExecuted == endExecutedExpected) {
      endExpectedExecutions.resolve();
    }
    assertEquals(result, expected);
  });

  const endTimeout = setTimeout(
    () => endExpectedExecutions.reject(),
    1000,
  );
  await endExpectedExecutions;
  clearTimeout(endTimeout);
  assertEquals(endExecuted, endExecutedExpected);
});

Deno.test("Readable stream: listeners can be removed", () => {
  const r = new Readable();
  r._read = () => {};
  r.on("data", () => {});

  r.removeAllListeners("data");

  assertEquals(r.eventNames().length, 0);
});
