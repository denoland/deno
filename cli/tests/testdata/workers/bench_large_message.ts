// Copyright 2020 the Deno authors. All rights reserved. MIT license.

function oneWorker(i: number) {
  return new Promise<void>((resolve) => {
    let countDown = 10;
    const worker = new Worker(
      new URL("worker_large_message.js", import.meta.url).href,
      { type: "module" },
    );
    worker.onmessage = (_e) => {
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

function bench() {
  const promises = [];
  for (let i = 0; i < 50; i++) {
    promises.push(oneWorker(i));
  }

  return Promise.all(promises);
}

bench();
