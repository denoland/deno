// Copyright Node.js contributors. All rights reserved. MIT License.
import { Buffer } from "../buffer.ts";
import PassThrough from "./passthrough.ts";
import pipeline from "./pipeline.ts";
import Readable from "./readable.ts";
import Transform from "./transform.ts";
import Writable from "./writable.ts";
import { mustCall } from "../_utils.ts";
import {
  assert,
  assertEquals,
  assertStrictEquals,
} from "../../testing/asserts.ts";
import type { NodeErrorAbstraction } from "../_errors.ts";

Deno.test("Pipeline ends on stream finished", async () => {
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

  const [finishedCompleted, finishedCb] = mustCall(
    (err?: NodeErrorAbstraction | null) => {
      assert(!err);
      assert(finished);
      assertEquals(processed, expected);
    },
    1,
  );

  pipeline(read, write, finishedCb);

  await finishedCompleted;
});

Deno.test("Pipeline fails on stream destroyed", async () => {
  const read = new Readable({
    read() {},
  });

  const write = new Writable({
    write(_data, _enc, cb) {
      cb();
    },
  });

  read.push("data");
  queueMicrotask(() => read.destroy());

  const [pipelineExecuted, pipelineCb] = mustCall(
    (err?: NodeErrorAbstraction | null) => {
      assert(err);
    },
    1,
  );
  pipeline(read, write, pipelineCb);

  await pipelineExecuted;
});

Deno.test("Pipeline exits on stream error", async () => {
  const read = new Readable({
    read() {},
  });

  const transform = new Transform({
    transform(_data, _enc, cb) {
      cb(new Error("kaboom"));
    },
  });

  const write = new Writable({
    write(_data, _enc, cb) {
      cb();
    },
  });

  const [readExecution, readCb] = mustCall();
  read.on("close", readCb);
  const [closeExecution, closeCb] = mustCall();
  transform.on("close", closeCb);
  const [writeExecution, writeCb] = mustCall();
  write.on("close", writeCb);

  const errorExecutions = [read, transform, write]
    .map((stream) => {
      const [execution, cb] = mustCall((err?: NodeErrorAbstraction | null) => {
        assertEquals(err, new Error("kaboom"));
      });

      stream.on("error", cb);
      return execution;
    });

  const [pipelineExecution, pipelineCb] = mustCall(
    (err?: NodeErrorAbstraction | null) => {
      assertEquals(err, new Error("kaboom"));
    },
  );
  const dst = pipeline(read, transform, write, pipelineCb);

  assertStrictEquals(dst, write);

  read.push("hello");

  await readExecution;
  await closeExecution;
  await writeExecution;
  await Promise.all(errorExecutions);
  await pipelineExecution;
});

Deno.test("Pipeline processes iterators correctly", async () => {
  let res = "";
  const w = new Writable({
    write(chunk, _encoding, callback) {
      res += chunk;
      callback();
    },
  });

  const [pipelineExecution, pipelineCb] = mustCall(
    (err?: NodeErrorAbstraction | null) => {
      assert(!err);
      assertEquals(res, "helloworld");
    },
  );
  pipeline(
    function* () {
      yield "hello";
      yield "world";
    }(),
    w,
    pipelineCb,
  );

  await pipelineExecution;
});

Deno.test("Pipeline processes async iterators correctly", async () => {
  let res = "";
  const w = new Writable({
    write(chunk, _encoding, callback) {
      res += chunk;
      callback();
    },
  });

  const [pipelineExecution, pipelineCb] = mustCall(
    (err?: NodeErrorAbstraction | null) => {
      assert(!err);
      assertEquals(res, "helloworld");
    },
  );
  pipeline(
    async function* () {
      await Promise.resolve();
      yield "hello";
      yield "world";
    }(),
    w,
    pipelineCb,
  );

  await pipelineExecution;
});

Deno.test("Pipeline processes generators correctly", async () => {
  let res = "";
  const w = new Writable({
    write(chunk, _encoding, callback) {
      res += chunk;
      callback();
    },
  });

  const [pipelineExecution, pipelineCb] = mustCall(
    (err?: NodeErrorAbstraction | null) => {
      assert(!err);
      assertEquals(res, "helloworld");
    },
  );
  pipeline(
    function* () {
      yield "hello";
      yield "world";
    },
    w,
    pipelineCb,
  );

  await pipelineExecution;
});

Deno.test("Pipeline processes async generators correctly", async () => {
  let res = "";
  const w = new Writable({
    write(chunk, _encoding, callback) {
      res += chunk;
      callback();
    },
  });

  const [pipelineExecution, pipelineCb] = mustCall(
    (err?: NodeErrorAbstraction | null) => {
      assert(!err);
      assertEquals(res, "helloworld");
    },
  );
  pipeline(
    async function* () {
      await Promise.resolve();
      yield "hello";
      yield "world";
    },
    w,
    pipelineCb,
  );

  await pipelineExecution;
});

Deno.test("Pipeline handles generator transforms", async () => {
  let res = "";

  const [pipelineExecuted, pipelineCb] = mustCall(
    (err?: NodeErrorAbstraction | null) => {
      assert(!err);
      assertEquals(res, "HELLOWORLD");
    },
  );
  pipeline(
    async function* () {
      await Promise.resolve();
      yield "hello";
      yield "world";
    },
    async function* (source: string[]) {
      for await (const chunk of source) {
        yield chunk.toUpperCase();
      }
    },
    async function (source: string[]) {
      for await (const chunk of source) {
        res += chunk;
      }
    },
    pipelineCb,
  );

  await pipelineExecuted;
});

Deno.test("Pipeline passes result to final callback", async () => {
  const [pipelineExecuted, pipelineCb] = mustCall(
    (err?: NodeErrorAbstraction | null, val?: unknown) => {
      assert(!err);
      assertEquals(val, "HELLOWORLD");
    },
  );
  pipeline(
    async function* () {
      await Promise.resolve();
      yield "hello";
      yield "world";
    },
    async function* (source: string[]) {
      for await (const chunk of source) {
        yield chunk.toUpperCase();
      }
    },
    async function (source: string[]) {
      let ret = "";
      for await (const chunk of source) {
        ret += chunk;
      }
      return ret;
    },
    pipelineCb,
  );

  await pipelineExecuted;
});

Deno.test("Pipeline returns a stream after ending", async () => {
  const [pipelineExecuted, pipelineCb] = mustCall(
    (err?: NodeErrorAbstraction | null) => {
      assertEquals(err, undefined);
    },
  );
  const ret = pipeline(
    async function* () {
      await Promise.resolve();
      yield "hello";
    },
    // deno-lint-ignore require-yield
    async function* (source: string[]) {
      for await (const chunk of source) {
        chunk;
      }
    },
    pipelineCb,
  );

  ret.resume();

  assertEquals(typeof ret.pipe, "function");

  await pipelineExecuted;
});

Deno.test("Pipeline returns a stream after erroring", async () => {
  const errorText = "kaboom";

  const [pipelineExecuted, pipelineCb] = mustCall(
    (err?: NodeErrorAbstraction | null) => {
      assertEquals(err?.message, errorText);
    },
  );
  const ret = pipeline(
    // deno-lint-ignore require-yield
    async function* () {
      await Promise.resolve();
      throw new Error(errorText);
    },
    // deno-lint-ignore require-yield
    async function* (source: string[]) {
      for await (const chunk of source) {
        chunk;
      }
    },
    pipelineCb,
  );

  ret.resume();

  assertEquals(typeof ret.pipe, "function");

  await pipelineExecuted;
});

Deno.test("Pipeline destination gets destroyed on error", async () => {
  const errorText = "kaboom";
  const s = new PassThrough();

  const [pipelineExecution, pipelineCb] = mustCall(
    (err?: NodeErrorAbstraction | null) => {
      assertEquals(err?.message, errorText);
      assertEquals(s.destroyed, true);
    },
  );
  pipeline(
    // deno-lint-ignore require-yield
    async function* () {
      throw new Error(errorText);
    },
    s,
    pipelineCb,
  );

  await pipelineExecution;
});
