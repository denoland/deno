// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertThrows } from "./test_util.ts";

Deno.test("websocketPermissionless", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "net" });

  assertThrows(
    () => new WebSocket("ws://localhost"),
    Deno.errors.PermissionDenied,
  );
});
