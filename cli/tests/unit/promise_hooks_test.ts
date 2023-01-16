// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "./test_util.ts";

function monitorPromises(outputArray: string[]) {
  const promiseIds = new Map<Promise<unknown>, string>();

  function identify(promise: Promise<unknown>) {
    if (!promiseIds.has(promise)) {
      promiseIds.set(promise, "p" + (promiseIds.size + 1));
    }
    return promiseIds.get(promise);
  }

  // @ts-ignore: Deno.core allowed
  Deno.core.setPromiseHooks(
    (promise: Promise<unknown>, parentPromise?: Promise<unknown>) => {
      outputArray.push(
        `init ${identify(promise)}` +
          (parentPromise ? ` from ${identify(parentPromise)}` : ``),
      );
    },
    (promise: Promise<unknown>) => {
      outputArray.push(`before ${identify(promise)}`);
    },
    (promise: Promise<unknown>) => {
      outputArray.push(`after ${identify(promise)}`);
    },
    (promise: Promise<unknown>) => {
      outputArray.push(`resolve ${identify(promise)}`);
    },
  );
}

Deno.test(async function promiseHookBasic() {
  // Bogus await here to ensure any pending promise resolution from the
  // test runtime has a chance to run and avoid contaminating our results.
  await Promise.resolve(null);

  const hookResults: string[] = [];
  monitorPromises(hookResults);

  async function asyncFn() {
    await Promise.resolve(15);
    await Promise.resolve(20);
    Promise.reject(new Error()).catch(() => {});
  }

  // The function above is equivalent to:
  //   function asyncFn() {
  //     return new Promise(resolve => {
  //       Promise.resolve(15).then(() => {
  //         Promise.resolve(20).then(() => {
  //           Promise.reject(new Error()).catch(() => {});
  //           resolve();
  //         });
  //       });
  //     });
  //   }

  await asyncFn();

  assertEquals(hookResults, [
    "init p1", // Creates the promise representing the return of `asyncFn()`.
    "init p2", // Creates the promise representing `Promise.resolve(15)`.
    "resolve p2", // The previous promise resolves to `15` immediately.
    "init p3 from p2", // Creates the promise that is resolved after the first `await` of the function. Equivalent to `p2.then(...)`.
    "init p4 from p1", // The resolution above gives time for other pending code to run. Creates the promise that is resolved
    // from the `await` at `await asyncFn()`, the last code to run. Equivalent to `asyncFn().then(...)`.
    "before p3", // Begins executing the code after `await Promise.resolve(15)`.
    "init p5", // Creates the promise representing `Promise.resolve(20)`.
    "resolve p5", // The previous promise resolves to `20` immediately.
    "init p6 from p5", // Creates the promise that is resolved after the second `await` of the function. Equivalent to `p5.then(...)`.
    "resolve p3", // The promise representing the code right after the first await is marked as resolved.
    "after p3", // We are now after the resolution code of the promise above.
    "before p6", // Begins executing the code after `await Promise.resolve(20)`.
    "init p7", // Creates a new promise representing `Promise.reject(new Error())`.
    "resolve p7", // This promise is "resolved" immediately to a rejection with an error instance.
    "init p8 from p7", // Creates a new promise for the `.catch` of the previous promise.
    "resolve p1", // At this point the promise of the function is resolved.
    "resolve p6", // This concludes the resolution of the code after `await Promise.resolve(20)`.
    "after p6", // We are now after the resolution code of the promise above.
    "before p8", // The `.catch` block is pending execution, it begins to execute.
    "resolve p8", // It does nothing and resolves to `undefined`.
    "after p8", // We are after the resolution of the `.catch` block.
    "before p4", // Now we begin the execution of the code that happens after `await asyncFn();`.
  ]);
});

Deno.test(async function promiseHookMultipleConsumers() {
  const hookResultsFirstConsumer: string[] = [];
  const hookResultsSecondConsumer: string[] = [];

  monitorPromises(hookResultsFirstConsumer);
  monitorPromises(hookResultsSecondConsumer);

  async function asyncFn() {
    await Promise.resolve(15);
    await Promise.resolve(20);
    Promise.reject(new Error()).catch(() => {});
  }
  await asyncFn();

  // Two invocations of `setPromiseHooks` should yield the exact same results, in the same order.
  assertEquals(
    hookResultsFirstConsumer,
    hookResultsSecondConsumer,
  );
});
