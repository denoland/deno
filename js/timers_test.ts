// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assertEqual } from "./test_util.ts";

test(async function timerTestArgs() {
  let arg = 1;
  await new Promise((resolve, reject) => {
    setTimeout((a, b, c) => {
      try {
        assertEqual(a, arg);
        assertEqual(b, arg.toString());
        assertEqual(c, [arg]);
        resolve();
      } catch (e) {
        reject(e);
      }
    }, 10, arg, arg.toString(), [arg]);
  });
});

test(async function timerTestClearedTimerId() {
  const timers = [];
  let timeouts = 0;
  for(let i = 0; i < 5; i++) {
    timers[i] = setTimeout(onTimeout, 10);
  }
  function onTimeout() {
    ++timeouts;
    for (let i = 1; i < timers.length; i++) {
      clearTimeout(timers[i]);
    }
  }
  await new Promise((resolve, reject) => {
    setTimeout(() => {
      try {
        assertEqual(timeouts, 1);
        resolve();
      } catch (e) {
        reject(e);
      }
    }, 200);
  });
});
