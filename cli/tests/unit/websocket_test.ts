// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertThrowsAsync, unitTest } from "./test_util.ts";

unitTest(function websocketPermissionDenied(): void {
  await assertThrowsAsync(async () => {
    const socket = new WebSocket("wss://gateway.discord.gg");
    socket.onmessage = (ev) => console.log(ev.data);
  }, Deno.errors.PermissionDenied);
});
