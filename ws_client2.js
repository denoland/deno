const ws = new WebSocket("ws://127.0.0.1:7000");

ws.addEventListener("error", console.error);

ws.addEventListener("open", function open() {
  ws.send("something");
  let i = 0;
  let id;
  id = setInterval(() => {
    i++;
    if (i > 2) {
      clearInterval(id);
      ws.close();
      return;
    }
    ws.send("hello " + i);
  }, 1000);
});

ws.addEventListener("message", function message(data) {
  console.log("received: %s", data);
});

ws.addEventListener("close", function close() {
  console.log("websocket closed");
});
