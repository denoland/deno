// Copyright Node.js contributors. All rights reserved. MIT License.
import { deferred } from "../../async/mod.ts";
import {
  assert,
  assertEquals,
  assertThrowsAsync,
} from "../../testing/asserts.ts";
import { Buffer } from "../buffer.ts";
import { pipeline } from "./promises.ts";
import Readable from "./readable.ts";
import Writable from "./writable.ts";

Deno.test("Promise pipeline works correctly", async () => {
  let pipelineExecuted = 0;
  const pipelineExecutedExpected = 1;
  const pipelineExpectedExecutions = deferred();

  let finished = false;
  // deno-lint-ignore no-explicit-any
  const processed: any[] = [];
  const expected = [
    Buffer.from("a"),
    Buffer.from("b"),
    Buffer.from("c"),
  ];

  const read = new Readable({
    read() {},
  });

  const write = new Writable({
    write(data, _enc, cb) {
      processed.push(data);
      cb();
    },
  });

  write.on("finish", () => {
    finished = true;
  });

  for (let i = 0; i < expected.length; i++) {
    read.push(expected[i]);
  }
  read.push(null);

  pipeline(read, write).then(() => {
    pipelineExecuted++;
    if (pipelineExecuted == pipelineExecutedExpected) {
      pipelineExpectedExecutions.resolve();
    }
    assert(finished);
    assertEquals(processed, expected);
  });

  const pipelineTimeout = setTimeout(
    () => pipelineExpectedExecutions.reject(),
    1000,
  );
  await pipelineExpectedExecutions;
  clearTimeout(pipelineTimeout);
  assertEquals(pipelineExecuted, pipelineExecutedExpected);
});

Deno.test("Promise pipeline throws on readable destroyed", async () => {
  const read = new Readable({
    read() {},
  });

  const write = new Writable({
    write(_data, _enc, cb) {
      cb();
    },
  });

  read.push("data");
  read.destroy();

  await assertThrowsAsync(
    () => pipeline(read, write),
    Error,
    "Premature close",
  );
});
