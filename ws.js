Deno.serve((req) => {
  const { socket, response } = Deno.upgradeWebSocket(req);
  socket.onmessage = (e) => {
    socket.send(e.data);
  };
  return response;
}, { port: 8000 });
