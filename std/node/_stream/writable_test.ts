// Copyright Node.js contributors. All rights reserved.

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
import { Buffer } from "../buffer.ts";
import finished from "./end-of-stream.ts";
import Writable from "../_stream/writable.ts";
import { deferred } from "../../async/mod.ts";
import {
  assert,
  assertEquals,
  assertStrictEquals,
  assertThrows,
} from "../../testing/asserts.ts";

Deno.test("Writable stream writes correctly", async () => {
  let callback: undefined | ((error?: Error | null | undefined) => void);

  let writeExecuted = 0;
  const writeExecutedExpected = 1;
  const writeExpectedExecutions = deferred();

  let writevExecuted = 0;
  const writevExecutedExpected = 1;
  const writevExpectedExecutions = deferred();

  const writable = new Writable({
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

Deno.test("Writable stream writes Uint8Array in object mode", async () => {
  let writeExecuted = 0;
  const writeExecutedExpected = 1;
  const writeExpectedExecutions = deferred();

  const ABC = new TextEncoder().encode("ABC");

  const writable = new Writable({
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

Deno.test("Writable stream throws on unexpected close", async () => {
  let finishedExecuted = 0;
  const finishedExecutedExpected = 1;
  const finishedExpectedExecutions = deferred();

  const writable = new Writable({
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

Deno.test("Writable stream finishes correctly", async () => {
  let finishedExecuted = 0;
  const finishedExecutedExpected = 1;
  const finishedExpectedExecutions = deferred();

  const w = new Writable({
    write(_chunk, _encoding, cb) {
      cb();
    },
    autoDestroy: false,
  });

  w.end("asd");

  queueMicrotask(() => {
    finished(w, () => {
      finishedExecuted++;
      if (finishedExecuted == finishedExecutedExpected) {
        finishedExpectedExecutions.resolve();
      }
    });
  });

  const finishedTimeout = setTimeout(
    () => finishedExpectedExecutions.reject(),
    1000,
  );
  await finishedExpectedExecutions;
  clearTimeout(finishedTimeout);
  assertEquals(finishedExecuted, finishedExecutedExpected);
});

Deno.test("Writable stream finishes correctly after error", async () => {
  let errorExecuted = 0;
  const errorExecutedExpected = 1;
  const errorExpectedExecutions = deferred();

  let finishedExecuted = 0;
  const finishedExecutedExpected = 1;
  const finishedExpectedExecutions = deferred();

  const w = new Writable({
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

Deno.test("Writable stream fails on 'write' null value", () => {
  const writable = new Writable();
  assertThrows(() => writable.write(null));
});
