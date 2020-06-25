# ws

ws module is made to provide helpers to create WebSocket client/server.

## Usage

### Server

```ts
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { serve } from "https://deno.land/std/http/server.ts";
import {
  acceptWebSocket,
  isWebSocketCloseEvent,
  isWebSocketPingEvent,
  WebSocket,
} from "https://deno.land/std/ws/mod.ts";

async function handleWs(sock: WebSocket) {
  console.log("socket connected!");
  try {
    for await (const ev of sock) {
      if (typeof ev === "string") {
        // text message
        console.log("ws:Text", ev);
        await sock.send(ev);
      } else if (ev instanceof Uint8Array) {
        // binary message
        console.log("ws:Binary", ev);
      } else if (isWebSocketPingEvent(ev)) {
        const [, body] = ev;
        // ping
        console.log("ws:Ping", body);
      } else if (isWebSocketCloseEvent(ev)) {
        // close
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

### Client

```ts
import {
  connectWebSocket,
  isWebSocketCloseEvent,
  isWebSocketPingEvent,
  isWebSocketPongEvent,
} from "https://deno.land/std/ws/mod.ts";
import { encode } from "https://deno.land/std/encoding/utf8.ts";
import { BufReader } from "https://deno.land/std/io/bufio.ts";
import { TextProtoReader } from "https://deno.land/std/textproto/mod.ts";
import { blue, green, red, yellow } from "https://deno.land/std/fmt/colors.ts";

const endpoint = Deno.args[0] || "ws://127.0.0.1:8080";
/** simple websocket cli */
try {
  const sock = await connectWebSocket(endpoint);
  console.log(green("ws connected! (type 'close' to quit)"));

  const messages = async (): Promise<void> => {
    for await (const msg of sock) {
      if (typeof msg === "string") {
        console.log(yellow(`< ${msg}`));
      } else if (isWebSocketPingEvent(msg)) {
        console.log(blue("< ping"));
      } else if (isWebSocketPongEvent(msg)) {
        console.log(blue("< pong"));
      } else if (isWebSocketCloseEvent(msg)) {
        console.log(red(`closed: code=${msg.code}, reason=${msg.reason}`));
      }
    }
  };

  const cli = async (): Promise<void> => {
    const tpr = new TextProtoReader(new BufReader(Deno.stdin));
    while (true) {
      await Deno.stdout.write(encode("> "));
      const line = await tpr.readLine();
      if (line === null || line === "close") {
        break;
      } else if (line === "ping") {
        await sock.ping();
      } else {
        await sock.send(line);
      }
    }
  };

  await Promise.race([messages(), cli()]).catch(console.error);

  if (!sock.isClosed) {
    await sock.close(1000).catch(console.error);
  }
} catch (err) {
  console.error(red(`Could not connect to WebSocket: '${err}'`));
}

Deno.exit(0);
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

Returns true if input headers are usable for WebSocket, otherwise false

### createSecAccept

Create value of Sec-WebSocket-Accept header from inputted nonce.

### acceptWebSocket

Upgrade inputted TCP connection into WebSocket connection.

### createSecKey

Returns base64 encoded 16 bytes string for Sec-WebSocket-Key header.

### connectWebSocket

Connect to WebSocket endpoint url with inputted endpoint string and headers.

- note: Endpoint must be acceptable for URL.
