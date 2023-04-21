Deno.serve((req) => {
  const { response, socket } = Deno.upgradeWebSocket(req);

  socket.onopen = () => socket.send("Hello");
  socket.onmessage = () => {
    socket.send("bye");
    socket.close();
  };
  socket.onerror = () => fail();
  return response;
});
