// Copyright Node.js contributors. All rights reserved. MIT License.
import { Buffer } from "../buffer.ts";
import Transform from "./transform.ts";
import finished from "./end_of_stream.ts";
import { Deferred } from "../../async/mod.ts";
import { assert, assertEquals } from "../../testing/asserts.ts";

Deno.test("Transform stream finishes correctly", async () => {
  let finishedExecuted = 0;
  const finishedExecutedExpected = 1;
  const finishedExecution = new Deferred<void>();

  const tr = new Transform({
    transform(_data, _enc, cb) {
      cb();
    },
  });

  let finish = false;
  let ended = false;

  tr.on("end", () => {
    ended = true;
  });

  tr.on("finish", () => {
    finish = true;
  });

  finished(tr, (err) => {
    finishedExecuted++;
    if (finishedExecuted === finishedExecutedExpected) {
      finishedExecution.resolve();
    }
    assert(!err, "no error");
    assert(finish);
    assert(ended);
  });

  tr.end();
  tr.resume();

  const finishedTimeout = setTimeout(
    () => finishedExecution.reject(),
    1000,
  );
  await finishedExecution;
  clearTimeout(finishedTimeout);
  assertEquals(finishedExecuted, finishedExecutedExpected);
});

Deno.test("Transform stream flushes data correctly", () => {
  const expected = "asdf";

  const t = new Transform({
    transform: (_d, _e, n) => {
      n();
    },
    flush: (n) => {
      n(null, expected);
    },
  });

  t.end(Buffer.from("blerg"));
  t.on("data", (data) => {
    assertEquals(data.toString(), expected);
  });
});
