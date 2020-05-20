// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assert, assertEquals, assertThrows } from "../testing/asserts.ts";
import { Queue } from "./queue.ts";
import { deferred, Deferred } from "../async/deferred.ts";

function assertQueue(
  queue: Queue<string>,
  expectedHeadVal: string | undefined,
  expectedSize: number
): void {
  assertEquals(queue.size(), expectedSize);
  assertEquals(queue.peek(), expectedHeadVal);
}

function queueToArray(queue: Queue<string>): string[] {
  const ret: string[] = [];
  for (const msg of queue.drain()) {
    ret.push(msg);
  }
  return ret;
}

test("Empty queue checks", function (): void {
  const queue: Queue<string> = new Queue<string>();
  assertQueue(queue, undefined, 0);
  assertEquals(queue.remove(), undefined);
  assertQueue(queue, undefined, 0);
  assertEquals(queueToArray(queue), []);
});

test("Add data to queue", function (): void {
  const queue: Queue<string> = new Queue<string>();
  queue.add("a");
  assertQueue(queue, "a", 1);
  queue.add("b");
  assertQueue(queue, "a", 2);
  queue.add("c");
  assertQueue(queue, "a", 3);
  assertEquals(queueToArray(queue), ["a", "b", "c"]);
});

test("remove data from queue", function (): void {
  const queue: Queue<string> = new Queue<string>();
  queue.add("a");
  queue.add("b");
  queue.add("c");
  assertQueue(queue, "a", 3);
  assertEquals(queue.remove(), "a");
  assertQueue(queue, "b", 2);
  assertEquals(queue.remove(), "b");
  assertQueue(queue, "c", 1);
  assertEquals(queue.remove(), "c");
  assertQueue(queue, undefined, 0);
  assertEquals(queueToArray(queue), []);
});

test("Async draining of queue consumes items and waits for new data", async function (): Promise<
  void
> {
  let dataProcessed: Deferred<void> = deferred();
  const queue: Queue<string> = new Queue<string>();
  const output: string[] = [];
  let drainComplete = false;
  const deferredDrainComplete: Deferred<void> = deferred();

  // Start draining, but first pause and wait on data to enter the queue
  (async (): Promise<void> => {
    for await (const msg of queue.drainAndWait()) {
      output.push(msg);
      dataProcessed.resolve();
    }
    drainComplete = true;
    deferredDrainComplete.resolve();
  })();

  queue.add("a");
  await dataProcessed;
  assertEquals(output, ["a"]);
  assert(queue.isEmpty());

  dataProcessed = deferred();

  //queue is drained.  Add more data to prove it will continue processing it.
  queue.add("b");
  queue.add("c");
  await dataProcessed; // wait for 'b' to process
  dataProcessed = deferred();
  await dataProcessed; // wait for 'c' to process
  assertEquals(output, ["a", "b", "c"]);
  assert(queue.isEmpty());

  dataProcessed = deferred();

  //now let's close the queue
  assert(!drainComplete);
  queue.add("d");
  queue.close();
  await dataProcessed;
  assertEquals(output, ["a", "b", "c", "d"]);
  assert(queue.isEmpty());

  //assert we drop out of the 'for await of' iterator
  await deferredDrainComplete;
  assert(drainComplete);
  assertEquals(queueToArray(queue), []);
});

test("Async draining of queue consumes items and completes", function (): void {
  const queue: Queue<string> = new Queue<string>();
  const output: string[] = [];

  queue.add("a");
  queue.add("b");
  queue.add("c");

  // This drain setup will drain the queue once and then quit the for/await/of loop
  for (const msg of queue.drain()) {
    output.push(msg);
  }

  assert(queue.isEmpty());
  assertEquals(output, ["a", "b", "c"]);

  //Add a final item to the queue and check it isn't processed by the drain
  queue.add("d");
  assertEquals(queue.size(), 1);
  assertEquals(output, ["a", "b", "c"]);
  assertEquals(queueToArray(queue), ["d"]);
});

test("Resetting queue discards queue items", function (): void {
  const queue: Queue<string> = new Queue<string>();
  queue.add("a");
  queue.add("b");
  queue.add("c");
  assertEquals(queue.size(), 3);
  assertEquals(queue.peek(), "a");

  queue.reset();

  assert(queue.isEmpty());
  assertEquals(queue.peek(), undefined);
});

test("Adding item to closed queue throws", function (): void {
  const queue: Queue<string> = new Queue<string>();
  queue.add("a");
  queue.close();
  assertThrows(
    () => {
      queue.add("b");
    },
    Error,
    "Queue is closed"
  );
});
