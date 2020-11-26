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
import { Readable, Transform, Writable } from "./stream.ts";
import { Buffer } from "./buffer.ts";
import { deferred } from "../async/mod.ts";
import { assert, assertEquals } from "../testing/asserts.ts";
import { mustCall } from "./_utils.ts";

Deno.test("Readable and Writable stream backpressure test", async () => {
  let pushes = 0;
  const total = 65500 + 40 * 1024;

  let rsExecuted = 0;
  const rsExecutedExpected = 11;
  const rsExpectedExecutions = deferred();

  let wsExecuted = 0;
  const wsExecutedExpected = 410;
  const wsExpectedExecutions = deferred();

  const rs = new Readable({
    read: function () {
      rsExecuted++;
      if (rsExecuted == rsExecutedExpected) {
        rsExpectedExecutions.resolve();
      }

      if (pushes++ === 10) {
        this.push(null);
        return;
      }

      assert(this._readableState.length <= total);

      this.push(Buffer.alloc(65500));
      for (let i = 0; i < 40; i++) {
        this.push(Buffer.alloc(1024));
      }
    },
  });

  const ws = new Writable({
    write: function (_data, _enc, cb) {
      wsExecuted++;
      if (wsExecuted == wsExecutedExpected) {
        wsExpectedExecutions.resolve();
      }
      cb();
    },
  });

  rs.pipe(ws);

  const rsTimeout = setTimeout(() => rsExpectedExecutions.reject(), 1000);
  const wsTimeout = setTimeout(() => wsExpectedExecutions.reject(), 1000);
  await rsExpectedExecutions;
  await wsExpectedExecutions;
  clearTimeout(rsTimeout);
  clearTimeout(wsTimeout);
  assertEquals(rsExecuted, rsExecutedExpected);
  assertEquals(wsExecuted, wsExecutedExpected);
});

Deno.test("Readable can be piped through Transform", async () => {
  const [readExecution, readCb] = mustCall(function (this: Readable) {
    this.push("content");
    this.push(null);
  });

  const r = new Readable({
    read: readCb,
  });

  const [transformExecution, transformCb] = mustCall(
    function (
      this: Transform,
      chunk: unknown,
      _e,
      callback: (error?: Error | null) => void,
    ) {
      this.push(chunk);
      callback();
    },
  );

  const [flushExecution, flushCb] = mustCall(
    function (this: Transform, callback: (error?: Error | null) => void) {
      callback();
    },
  );

  const t = new Transform({
    transform: transformCb,
    flush: flushCb,
  });

  r.pipe(t);

  const [readableExecution, readableCb] = mustCall(function () {
    while (true) {
      const chunk = t.read();
      if (!chunk) {
        break;
      }

      assertEquals(chunk.toString(), "content");
    }
  }, 2);

  t.on("readable", readableCb);

  await readExecution;
  await transformExecution;
  await flushExecution;
  await readableExecution;
});
