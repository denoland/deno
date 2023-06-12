import WebSocket from "npm:ws";

const ws = new WebSocket("ws://127.0.0.1:7000");

ws.on("error", console.error);

ws.on("open", function open() {
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

ws.on("message", function message(data) {
  console.log("received: %s", data);
});

ws.on("close", function close() {
  console.log("websocket closed");
});
