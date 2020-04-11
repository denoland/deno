#!/usr/bin/env -S deno run --reload --allow-run
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import "./unit_tests.ts";
import {
  registerUnitTests,
  assert,
  serializeTestMessage,
} from "./test_util.ts";

function onTestMessage(message: Deno.TestMessage): void {
  self.postMessage({
    testMsg: serializeTestMessage(message),
  });
}

self.onmessage = async function (e): Promise<void> {
  const { cmd, filter } = e.data;
  assert(cmd === "run");
  // Register unit tests that match process permissions
  await registerUnitTests();
  // Execute tests
  await Deno.runTests({
    exitOnFail: false,
    filter,
    reportToConsole: false,
    onMessage: onTestMessage,
  });
};
