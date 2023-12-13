// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { EventEmitter } from "node:events";

EventEmitter.captureRejections = true;

Deno.test("regression #20441", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();

  const ee = new EventEmitter();

  ee.on("foo", function () {
    const p = new Promise((_resolve, reject) => {
      setTimeout(() => {
        reject();
      }, 100);
    });
    return p;
  });

  ee.on("error", function (_) {
    resolve();
  });

  ee.emit("foo");
  await promise;
});
