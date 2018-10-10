// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";
import * as deno from "deno";

test(function metrics() {
  // TODO: metrics command does not send any data buffer to privileged side
  // should we call other method that does to present it here?

  const metrics1 = deno.metrics();

  assert(metrics1.opsExecuted > 0);
  assert(metrics1.controlBytesSent > 0);
  assert(metrics1.dataBytesSent >= 0);
  assert(metrics1.bytesReceived > 0);

  const metrics2 = deno.metrics();

  assert(metrics2.opsExecuted > metrics1.opsExecuted);
  assert(metrics2.controlBytesSent > metrics1.controlBytesSent);
  assert(metrics2.dataBytesSent >= metrics1.dataBytesSent);
  assert(metrics2.bytesReceived > metrics1.bytesReceived);
});
