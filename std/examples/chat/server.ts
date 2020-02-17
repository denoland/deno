import { serve } from "../../http/server.ts";
import {
  acceptWebSocket,
  acceptable,
  WebSocket,
  isWebSocketCloseEvent
} from "../../ws/mod.ts";

const clients = new Map<number, WebSocket>();
let clientId = 0;
async function dispatch(msg: string): Promise<void> {
  for (const client of clients.values()) {
    client.send(msg);
  }
}
async function wsHandler(ws: WebSocket): Promise<void> {
  const id = ++clientId;
  clients.set(id, ws);
  dispatch(`Connected: [${id}]`);
  for await (const msg of ws.receive()) {
    console.log(`msg:${id}`, msg);
    if (typeof msg === "string") {
      dispatch(`[${id}]: ${msg}`);
    } else if (isWebSocketCloseEvent(msg)) {
      clients.delete(id);
      dispatch(`Closed: [${id}]`);
      break;
    }
  }
}

async function main(): Promise<void> {
  console.log("chat server starting on :8080....");
  for await (const req of serve({ port: 8080 })) {
    if (req.method === "GET" && req.url === "/") {
      Deno.open("./index.html").then(file => {
        req
          .respond({
            status: 200,
            headers: new Headers({
              "content-type": "text/html"
            }),
            body: file
          })
          .finally(() => file.close());
      });
    }
    if (req.method === "GET" && req.url === "/ws") {
      if (acceptable(req)) {
        acceptWebSocket({
          conn: req.conn,
          bufReader: req.r,
          bufWriter: req.w,
          headers: req.headers
        }).then(wsHandler);
      }
    }
  }
}
main();
