Deno.serve({ port: 7000 }, (req) => {
  const { socket, response } = Deno.upgradeWebSocket(req, {
    idleTimeout: 0,
  });
  let i = 0;
  socket.onmessage = (e) => {
    if (i == 2) {
      socket.close();
      return;
    }
    i++;
    socket.send(e.data);
  };
  return response;
});
