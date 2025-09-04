// Copyright 2018-2025 the Deno authors. MIT license.
import { parseArgs } from "@std/cli/parse-args";

const { port } = parseArgs(Deno.args, {
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
