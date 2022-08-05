Deno.serve((request) => {
  const {
    response,
    socket,
  } = Deno.upgradeWebSocket(request);
  socket.onerror = () => console.log("error");
  socket.onmessage = (m) => {
    console.log(m);
    socket.send(m.data);
    socket.close(1001);
  };
  return response;
});
