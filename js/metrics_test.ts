// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";
import * as deno from "deno";

test(function metrics() {
  const metrics1 = deno.metrics();

  assert(metrics1.opsExecuted > 0);
  assert(metrics1.controlBytesSent > 0);
  assert(metrics1.dataBytesSent >= 0);
  assert(metrics1.bytesReceived > 0);

  // write something to ensure data bytes are sent to privileged side
  deno.stdout.write(new Uint8Array([41, 42, 43]));

  const metrics2 = deno.metrics();

  assert(metrics2.opsExecuted > metrics1.opsExecuted);
  assert(metrics2.controlBytesSent > metrics1.controlBytesSent);
  assert(metrics2.dataBytesSent > metrics1.dataBytesSent);
  assert(metrics2.bytesReceived > metrics1.bytesReceived);
});
