// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { deferred } from "../../../test_util/std/async/deferred.ts";
import { EventEmitter } from "node:events";

EventEmitter.captureRejections = true;

Deno.test("regression #20441", async () => {
  const promise = deferred();

  const ee = new EventEmitter();

  ee.on("foo", function () {
    const p = new Promise((resolve, reject) => {
      setTimeout(() => {
        reject();
      }, 100);
    });
    return p;
  });

  ee.on("error", function (_) {
    promise.resolve();
  });

  ee.emit("foo");
  await promise;
});
