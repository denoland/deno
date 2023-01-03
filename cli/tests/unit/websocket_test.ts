// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, deferred, fail } from "./test_util.ts";

Deno.test({ permissions: "none" }, function websocketPermissionless() {
  assertThrows(
    () => new WebSocket("ws://localhost"),
    Deno.errors.PermissionDenied,
  );
});

Deno.test(async function websocketConstructorTakeURLObjectAsParameter() {
  const promise = deferred();
  const ws = new WebSocket(new URL("ws://localhost:4242/"));
  assertEquals(ws.url, "ws://localhost:4242/");
  ws.onerror = () => fail();
  ws.onopen = () => ws.close();
  ws.onclose = () => {
    promise.resolve();
  };
  await promise;
});
