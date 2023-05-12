// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../../../test_util/std/testing/asserts.ts";
import workerThreads from "node:worker_threads";

Deno.test("[node/worker_threads] BroadcastChannel is exported", () => {
  assertEquals<unknown>(workerThreads.BroadcastChannel, BroadcastChannel);
});

Deno.test("[node/worker_threads] MessageChannel are MessagePort are exported", () => {
  assertEquals<unknown>(workerThreads.MessageChannel, MessageChannel);
  assertEquals<unknown>(workerThreads.MessagePort, MessagePort);
});
