// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertThrows } from "./test_util.ts";

Deno.test({ permissions: "none" }, function websocketPermissionless() {
  assertThrows(
    () => new WebSocket("ws://localhost"),
    Deno.errors.PermissionDenied,
  );
});
