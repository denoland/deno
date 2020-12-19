// Copyright Node.js contributors. All rights reserved. MIT License.
import { Buffer } from "../buffer.ts";
import Duplex from "./duplex.ts";
import finished from "./end_of_stream.ts";
import {
  assert,
  assertEquals,
  assertStrictEquals,
  assertThrows,
} from "../../testing/asserts.ts";
import { deferred, delay } from "../../async/mod.ts";

Deno.test("Duplex stream works normally", () => {
  const stream = new Duplex({ objectMode: true });

  assert(stream._readableState.objectMode);
  assert(stream._writableState.objectMode);
  assert(stream.allowHalfOpen);
  assertEquals(stream.listenerCount("end"), 0);

  let written: { val: number };
  let read: { val: number };

  stream._write = (obj, _, cb) => {
    written = obj;
    cb();
  };

  stream._read = () => {};

  stream.on("data", (obj) => {
    read = obj;
  });

  stream.push({ val: 1 });
  stream.end({ val: 2 });

  stream.on("finish", () => {
    assertEquals(read.val, 1);
    assertEquals(written.val, 2);
  });
});

Deno.test("Duplex stream gets constructed correctly", () => {
  const d1 = new Duplex({
    objectMode: true,
    highWaterMark: 100,
  });

  assertEquals(d1.readableObjectMode, true);
  assertEquals(d1.readableHighWaterMark, 100);
  assertEquals(d1.writableObjectMode, true);
  assertEquals(d1.writableHighWaterMark, 100);

  const d2 = new Duplex({
    readableObjectMode: false,
    readableHighWaterMark: 10,
    writableObjectMode: true,
    writableHighWaterMark: 100,
  });

  assertEquals(d2.writableObjectMode, true);
  assertEquals(d2.writableHighWaterMark, 100);
  assertEquals(d2.readableObjectMode, false);
  assertEquals(d2.readableHighWaterMark, 10);
});

Deno.test("Duplex stream can be paused", () => {
  const readable = new Duplex();

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

Deno.test("Duplex stream sets enconding correctly", () => {
  const readable = new Duplex({
    read() {},
  });

  readable.setEncoding("utf8");

  readable.push(new TextEncoder().encode("DEF"));
  readable.unshift(new TextEncoder().encode("ABC"));

  assertStrictEquals(readable.read(), "ABCDEF");
});

Deno.test("Duplex stream sets encoding correctly", () => {
  const readable = new Duplex({
    read() {},
  });

  readable.setEncoding("utf8");

  readable.push(new TextEncoder().encode("DEF"));
  readable.unshift(new TextEncoder().encode("ABC"));

  assertStrictEquals(readable.read(), "ABCDEF");
});

Deno.test("Duplex stream holds up a big push", async () => {
  let readExecuted = 0;
  const readExecutedExpected = 3;
  const readExpectedExecutions = deferred();

  let endExecuted = 0;
  const endExecutedExpected = 1;
  const endExpectedExecutions = deferred();

  const str = "asdfasdfasdfasdfasdf";

  const r = new Duplex({
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

Deno.test("Duplex stream: 'readable' event is emitted but 'read' is not on highWaterMark length exceeded", async () => {
  let readableExecuted = 0;
  const readableExecutedExpected = 1;
  const readableExpectedExecutions = deferred();

  const r = new Duplex({
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

Deno.test("Duplex stream: 'readable' and 'read' events are emitted on highWaterMark length not reached", async () => {
  let readableExecuted = 0;
  const readableExecutedExpected = 1;
  const readableExpectedExecutions = deferred();

  let readExecuted = 0;
  const readExecutedExpected = 1;
  const readExpectedExecutions = deferred();

  const r = new Duplex({
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

Deno.test("Duplex stream: 'readable' event is emitted but 'read' is not on highWaterMark length not reached and stream ended", async () => {
  let readableExecuted = 0;
  const readableExecutedExpected = 1;
  const readableExpectedExecutions = deferred();

  const r = new Duplex({
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

Deno.test("Duplex stream: 'read' is emitted on empty string pushed in non-object mode", async () => {
  let endExecuted = 0;
  const endExecutedExpected = 1;
  const endExpectedExecutions = deferred();

  const underlyingData = ["", "x", "y", "", "z"];
  const expected = underlyingData.filter((data) => data);
  const result: unknown[] = [];

  const r = new Duplex({
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

Deno.test("Duplex stream: listeners can be removed", () => {
  const r = new Duplex();
  r._read = () => {};
  r.on("data", () => {});

  r.removeAllListeners("data");

  assertEquals(r.eventNames().length, 0);
});

Deno.test("Duplex stream writes correctly", async () => {
  let callback: undefined | ((error?: Error | null | undefined) => void);

  let writeExecuted = 0;
  const writeExecutedExpected = 1;
  const writeExpectedExecutions = deferred();

  let writevExecuted = 0;
  const writevExecutedExpected = 1;
  const writevExpectedExecutions = deferred();

  const writable = new Duplex({
    write: (chunk, encoding, cb) => {
      writeExecuted++;
      if (writeExecuted == writeExecutedExpected) {
        writeExpectedExecutions.resolve();
      }
      assert(chunk instanceof Buffer);
      assertStrictEquals(encoding, "buffer");
      assertStrictEquals(String(chunk), "ABC");
      callback = cb;
    },
    writev: (chunks) => {
      writevExecuted++;
      if (writevExecuted == writevExecutedExpected) {
        writevExpectedExecutions.resolve();
      }
      assertStrictEquals(chunks.length, 2);
      assertStrictEquals(chunks[0].encoding, "buffer");
      assertStrictEquals(chunks[1].encoding, "buffer");
      assertStrictEquals(chunks[0].chunk + chunks[1].chunk, "DEFGHI");
    },
  });

  writable.write(new TextEncoder().encode("ABC"));
  writable.write(new TextEncoder().encode("DEF"));
  writable.end(new TextEncoder().encode("GHI"));
  callback?.();

  const writeTimeout = setTimeout(
    () => writeExpectedExecutions.reject(),
    1000,
  );
  const writevTimeout = setTimeout(
    () => writevExpectedExecutions.reject(),
    1000,
  );
  await writeExpectedExecutions;
  await writevExpectedExecutions;
  clearTimeout(writeTimeout);
  clearTimeout(writevTimeout);
  assertEquals(writeExecuted, writeExecutedExpected);
  assertEquals(writevExecuted, writevExecutedExpected);
});

Deno.test("Duplex stream writes Uint8Array in object mode", async () => {
  let writeExecuted = 0;
  const writeExecutedExpected = 1;
  const writeExpectedExecutions = deferred();

  const ABC = new TextEncoder().encode("ABC");

  const writable = new Duplex({
    objectMode: true,
    write: (chunk, encoding, cb) => {
      writeExecuted++;
      if (writeExecuted == writeExecutedExpected) {
        writeExpectedExecutions.resolve();
      }
      assert(!(chunk instanceof Buffer));
      assert(chunk instanceof Uint8Array);
      assertEquals(chunk, ABC);
      assertEquals(encoding, "utf8");
      cb();
    },
  });

  writable.end(ABC);

  const writeTimeout = setTimeout(
    () => writeExpectedExecutions.reject(),
    1000,
  );
  await writeExpectedExecutions;
  clearTimeout(writeTimeout);
  assertEquals(writeExecuted, writeExecutedExpected);
});

Deno.test("Duplex stream throws on unexpected close", async () => {
  let finishedExecuted = 0;
  const finishedExecutedExpected = 1;
  const finishedExpectedExecutions = deferred();

  const writable = new Duplex({
    write: () => {},
  });
  writable.writable = false;
  writable.destroy();

  finished(writable, (err) => {
    finishedExecuted++;
    if (finishedExecuted == finishedExecutedExpected) {
      finishedExpectedExecutions.resolve();
    }
    assertEquals(err?.code, "ERR_STREAM_PREMATURE_CLOSE");
  });

  const finishedTimeout = setTimeout(
    () => finishedExpectedExecutions.reject(),
    1000,
  );
  await finishedExpectedExecutions;
  clearTimeout(finishedTimeout);
  assertEquals(finishedExecuted, finishedExecutedExpected);
});

Deno.test("Duplex stream finishes correctly after error", async () => {
  let errorExecuted = 0;
  const errorExecutedExpected = 1;
  const errorExpectedExecutions = deferred();

  let finishedExecuted = 0;
  const finishedExecutedExpected = 1;
  const finishedExpectedExecutions = deferred();

  const w = new Duplex({
    write(_chunk, _encoding, cb) {
      cb(new Error());
    },
    autoDestroy: false,
  });
  w.write("asd");
  w.on("error", () => {
    errorExecuted++;
    if (errorExecuted == errorExecutedExpected) {
      errorExpectedExecutions.resolve();
    }
    finished(w, () => {
      finishedExecuted++;
      if (finishedExecuted == finishedExecutedExpected) {
        finishedExpectedExecutions.resolve();
      }
    });
  });

  const errorTimeout = setTimeout(
    () => errorExpectedExecutions.reject(),
    1000,
  );
  const finishedTimeout = setTimeout(
    () => finishedExpectedExecutions.reject(),
    1000,
  );
  await finishedExpectedExecutions;
  await errorExpectedExecutions;
  clearTimeout(finishedTimeout);
  clearTimeout(errorTimeout);
  assertEquals(finishedExecuted, finishedExecutedExpected);
  assertEquals(errorExecuted, errorExecutedExpected);
});

Deno.test("Duplex stream fails on 'write' null value", () => {
  const writable = new Duplex();
  assertThrows(() => writable.write(null));
});

Deno.test("Duplex stream is destroyed correctly", async () => {
  let closeExecuted = 0;
  const closeExecutedExpected = 1;
  const closeExpectedExecutions = deferred();

  const unexpectedExecution = deferred();

  const duplex = new Duplex({
    write(_chunk, _enc, cb) {
      cb();
    },
    read() {},
  });

  duplex.resume();

  function never() {
    unexpectedExecution.reject();
  }

  duplex.on("end", never);
  duplex.on("finish", never);
  duplex.on("close", () => {
    closeExecuted++;
    if (closeExecuted == closeExecutedExpected) {
      closeExpectedExecutions.resolve();
    }
  });

  duplex.destroy();
  assertEquals(duplex.destroyed, true);

  const closeTimeout = setTimeout(
    () => closeExpectedExecutions.reject(),
    1000,
  );
  await Promise.race([
    unexpectedExecution,
    delay(100),
  ]);
  await closeExpectedExecutions;
  clearTimeout(closeTimeout);
  assertEquals(closeExecuted, closeExecutedExpected);
});

Deno.test("Duplex stream errors correctly on destroy", async () => {
  let errorExecuted = 0;
  const errorExecutedExpected = 1;
  const errorExpectedExecutions = deferred();

  const unexpectedExecution = deferred();

  const duplex = new Duplex({
    write(_chunk, _enc, cb) {
      cb();
    },
    read() {},
  });
  duplex.resume();

  const expected = new Error("kaboom");

  function never() {
    unexpectedExecution.reject();
  }

  duplex.on("end", never);
  duplex.on("finish", never);
  duplex.on("error", (err) => {
    errorExecuted++;
    if (errorExecuted == errorExecutedExpected) {
      errorExpectedExecutions.resolve();
    }
    assertStrictEquals(err, expected);
  });

  duplex.destroy(expected);
  assertEquals(duplex.destroyed, true);

  const errorTimeout = setTimeout(
    () => errorExpectedExecutions.reject(),
    1000,
  );
  await Promise.race([
    unexpectedExecution,
    delay(100),
  ]);
  await errorExpectedExecutions;
  clearTimeout(errorTimeout);
  assertEquals(errorExecuted, errorExecutedExpected);
});

Deno.test("Duplex stream doesn't finish on allowHalfOpen", async () => {
  const unexpectedExecution = deferred();

  const duplex = new Duplex({
    read() {},
  });

  assertEquals(duplex.allowHalfOpen, true);
  duplex.on("finish", () => unexpectedExecution.reject());
  assertEquals(duplex.listenerCount("end"), 0);
  duplex.resume();
  duplex.push(null);

  await Promise.race([
    unexpectedExecution,
    delay(100),
  ]);
});

Deno.test("Duplex stream finishes when allowHalfOpen is disabled", async () => {
  let finishExecuted = 0;
  const finishExecutedExpected = 1;
  const finishExpectedExecutions = deferred();

  const duplex = new Duplex({
    read() {},
    allowHalfOpen: false,
  });

  assertEquals(duplex.allowHalfOpen, false);
  duplex.on("finish", () => {
    finishExecuted++;
    if (finishExecuted == finishExecutedExpected) {
      finishExpectedExecutions.resolve();
    }
  });
  assertEquals(duplex.listenerCount("end"), 0);
  duplex.resume();
  duplex.push(null);

  const finishTimeout = setTimeout(
    () => finishExpectedExecutions.reject(),
    1000,
  );
  await finishExpectedExecutions;
  clearTimeout(finishTimeout);
  assertEquals(finishExecuted, finishExecutedExpected);
});

Deno.test("Duplex stream doesn't finish when allowHalfOpen is disabled but stream ended", async () => {
  const unexpectedExecution = deferred();

  const duplex = new Duplex({
    read() {},
    allowHalfOpen: false,
  });

  assertEquals(duplex.allowHalfOpen, false);
  duplex._writableState.ended = true;
  duplex.on("finish", () => unexpectedExecution.reject());
  assertEquals(duplex.listenerCount("end"), 0);
  duplex.resume();
  duplex.push(null);

  await Promise.race([
    unexpectedExecution,
    delay(100),
  ]);
});
