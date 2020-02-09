// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assertEquals } from "./test_util.ts";
import { runIfMain } from "../../std/testing/mod.ts";

export interface ResolvableMethods<T> {
  resolve: (value?: T | PromiseLike<T>) => void;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  reject: (reason?: any) => void;
}

export type Resolvable<T> = Promise<T> & ResolvableMethods<T>;

export function createResolvable<T>(): Resolvable<T> {
  let methods: ResolvableMethods<T>;
  const promise = new Promise<T>((resolve, reject): void => {
    methods = { resolve, reject };
  });
  // TypeScript doesn't know that the Promise callback occurs synchronously
  // therefore use of not null assertion (`!`)
  return Object.assign(promise, methods!) as Resolvable<T>;
}

test(async function workersBasic(): Promise<void> {
  const promise = createResolvable();
  console.log("creating worker!!!!");
  const jsWorker = new Worker("../tests/subdir/test_worker.js", {
    type: "module",
    name: "jsWorker"
  });
  console.log("created worker !!!!");
  const tsWorker = new Worker("../tests/subdir/test_worker.ts", {
    type: "module",
    name: "tsWorker"
  });

  tsWorker.onmessage = (e): void => {
    assertEquals(e.data, "Hello World");
    promise.resolve();
  };

  jsWorker.onmessage = (e): void => {
    assertEquals(e.data, "Hello World");
    tsWorker.postMessage("Hello World");
  };

  jsWorker.onerror = (e: Event): void => {
    console.log("on error in jsWorker");
    e.preventDefault();
    jsWorker.postMessage("Hello World");
  };

  console.log("before!!!!");
  jsWorker.postMessage("Hello World");
  await promise;
  console.log("promise resolved :)");
});

/* FIXME(bartlomieju)
test(async function nestedWorker(): Promise<void> {
  const promise = createResolvable();

  const nestedWorker = new Worker("../tests/subdir/nested_worker.js", {
    type: "module",
    name: "nested",
  });

  nestedWorker.onmessage = (e): void => {
    assert(e.data.type !== "error");
    promise.resolve();
  };

  nestedWorker.postMessage("Hello World");
  await promise;
  console.log("promise resolved :)");
});
*/

// test(async function workerThrowsWhenExecuting(): Promise<void> {
//   const promise = createResolvable();

//   const throwingWorker = new Worker("../tests/subdir/throwing_worker.js", {
//     type: "module"
//   });

//   // eslint-disable-next-line @typescript-eslint/no-explicit-any
//   throwingWorker.onerror = (e: any): void => {
//     e.preventDefault();
//     assertEquals(e.message, "Uncaught Error: Thrown error");
//     promise.resolve();
//   };

//   console.log("before!!!!");
//   await promise;
//   console.log("promise resolved :)");
//   console.table(Deno.metrics());
// });

runIfMain(import.meta);
