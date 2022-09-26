// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

Deno.serve((r) => {
  const { response, socket } = Deno.upgradeWebSocket(r);
  socket.onmessage = (e) => {
    socket.send(e.data);
  };
  socket.onopen = () => {
    socket.send("open");
  };
  return response;
}, { port: 8000 });
