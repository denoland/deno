// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";
import * as deno from "deno";

test(function metrics() {
  const m1 = deno.metrics();
  assert(m1.opsDispatched > 0);
  assert(m1.opsCompleted > 0);
  assert(m1.bytesSentControl > 0);
  assert(m1.bytesSentData >= 0);
  assert(m1.bytesReceived > 0);

  // Write to stdout to ensure a "data" message gets sent instead of just
  // control messages.
  const dataMsg = new Uint8Array([41, 42, 43]);
  deno.stdout.write(dataMsg);

  const m2 = deno.metrics();
  assert(m2.opsDispatched > m1.opsDispatched);
  assert(m2.opsCompleted > m1.opsCompleted);
  assert(m2.bytesSentControl > m1.bytesSentControl);
  assert(m2.bytesSentData >= m1.bytesSentData + dataMsg.byteLength);
  assert(m2.bytesReceived > m1.bytesReceived);
});
