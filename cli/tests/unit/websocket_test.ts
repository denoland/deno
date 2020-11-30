// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertThrows, unitTest } from "./test_util.ts";

unitTest(function websocketPermissionless() {
  assertThrows(
    () => new WebSocket("ws://localhost"),
    Deno.errors.PermissionDenied,
  );
});
