// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { parse } from "../../../test_util/std/flags/mod.ts";

const { port } = parse(Deno.args, {
  number: ["port"],
  default: {
    port: 6969,
  },
});

const { serve } = Deno;

// A message-based WebSocket echo server.
serve({ port }, (request) => {
  const { socket, response } = Deno.upgradeWebSocket(request);
  socket.onmessage = (event) => {
    socket.send(event.data);
  };
  return response;
});
