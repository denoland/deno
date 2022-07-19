const httpConn = new Deno.flash.HttpConn();

for await (const reqEvent of httpConn) {
  const { request, respondWith } = reqEvent;
  const {
    response,
    socket,
  } = Deno.upgradeWebSocket(request);
  console.log(socket);
  socket.onerror = () => console.log("error");
  socket.onmessage = (m) => {
    console.log(m);
    socket.send(m.data);
    socket.close(1001);
  };
  await respondWith(response);
  console.log(socket);
}
