Deno.serve({
  port: 4319,
  onListen() {
    console.log("READY");
  },
  handler(request) {
    const { response, socket } = Deno.upgradeWebSocket(request);
    socket.onerror = () => Deno.exit(1);
    socket.onopen = () => socket.close();
    socket.onclose = () => Deno.exit();
    return response;
  },
});
