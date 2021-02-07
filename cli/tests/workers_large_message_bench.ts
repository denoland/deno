// Copyright 2020 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

import { deferred } from "../../test_util/std/async/deferred.ts";

function oneWorker(i: any): Promise<void> {
  return new Promise<void>((resolve) => {
    let countDown = 10;
    const worker = new Worker(
      new URL("workers/large_message_worker.ts", import.meta.url).href,
      { type: "module" },
    );
    worker.onmessage = (e): void => {
      if (countDown > 0) {
        countDown--;
        return;
      }
      worker.terminate();
      resolve();
    };
    worker.postMessage("hi " + i);
  });
}

function bench(): Promise<any> {
  let promises = [];
  for (let i = 0; i < 50; i++) {
    promises.push(oneWorker(i));
  }

  return Promise.all(promises);
}

bench();
