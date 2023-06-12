import { WebSocketServer } from "npm:ws";

const wss = new WebSocketServer({ port: 7000 });
console.log("Listening on http://127.0.0.1:7000");

wss.on("connection", function connection(ws) {
  ws.on("error", console.error);

  ws.on("message", function message(data) {
    console.log("received: %s", data);
  });

  ws.send("something");
});
