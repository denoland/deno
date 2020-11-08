# ws

ws module is made to provide helpers to create WebSocket server. For client
WebSockets, use the
[WebSocket API](https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API).

## Usage

```ts
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { serve } from "https://deno.land/std@$STD_VERSION/http/server.ts";
import {
  acceptWebSocket,
  isWebSocketCloseEvent,
  isWebSocketPingEvent,
  WebSocket,
} from "https://deno.land/std@$STD_VERSION/ws/mod.ts";

async function handleWs(sock: WebSocket) {
  console.log("socket connected!");
  try {
    for await (const ev of sock) {
      if (typeof ev === "string") {
        // text message.
        console.log("ws:Text", ev);
        await sock.send(ev);
      } else if (ev instanceof Uint8Array) {
        // binary message.
        console.log("ws:Binary", ev);
      } else if (isWebSocketPingEvent(ev)) {
        const [, body] = ev;
        // ping.
        console.log("ws:Ping", body);
      } else if (isWebSocketCloseEvent(ev)) {
        // close.
        const { code, reason } = ev;
        console.log("ws:Close", code, reason);
      }
    }
  } catch (err) {
    console.error(`failed to receive frame: ${err}`);

    if (!sock.isClosed) {
      await sock.close(1000).catch(console.error);
    }
  }
}

if (import.meta.main) {
  /** websocket echo server */
  const port = Deno.args[0] || "8080";
  console.log(`websocket server is running on :${port}`);
  for await (const req of serve(`:${port}`)) {
    const { conn, r: bufReader, w: bufWriter, headers } = req;
    acceptWebSocket({
      conn,
      bufReader,
      bufWriter,
      headers,
    })
      .then(handleWs)
      .catch(async (err) => {
        console.error(`failed to accept websocket: ${err}`);
        await req.respond({ status: 400 });
      });
  }
}
```

## API

### isWebSocketCloseEvent

Returns true if input value is a WebSocketCloseEvent, false otherwise.

### isWebSocketPingEvent

Returns true if input value is a WebSocketPingEvent, false otherwise.

### isWebSocketPongEvent

Returns true if input value is a WebSocketPongEvent, false otherwise.

### unmask

Unmask masked WebSocket payload.

### writeFrame

Write WebSocket frame to inputted writer.

### readFrame

Read WebSocket frame from inputted BufReader.

### createMask

Create mask from the client to the server with random 32bit number.

### acceptable

Returns true if input headers are usable for WebSocket, otherwise false.

### createSecAccept

Create value of Sec-WebSocket-Accept header from inputted nonce.

### acceptWebSocket

Upgrade inputted TCP connection into WebSocket connection.

### createSecKey

Returns base64 encoded 16 bytes string for Sec-WebSocket-Key header.
