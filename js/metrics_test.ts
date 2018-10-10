// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";
import * as deno from "deno";

test(function metrics() {
  const metrics1 = deno.metrics();
  assert(metrics1.opsExecuted > 0);
  assert(metrics1.bytesRecv > 0);
  assert(metrics1.bytesSent > 0);

  const metrics2 = deno.metrics();
  assert(metrics2.opsExecuted > metrics1.opsExecuted);
  assert(metrics2.bytesSent > metrics1.bytesRecv);
  assert(metrics2.bytesSent > metrics1.bytesSent);
});
