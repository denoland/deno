// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const port = Deno.args[0] ?? "8080";
const { serve } = Deno;

function handler(request) {
  const { socket, response } = Deno.upgradeWebSocket(request, {
    idleTimeout: 0,
  });
  socket.onmessage = (e) => {
    socket.send(e.data);
  };

  socket.onopen = () => {
    console.log("Connected to client");
  };

  socket.onerror = (e) => {
    console.log(e);
  };

  return response;
}

serve(handler, { port: parseInt(port), hostname: "0.0.0.0" });
