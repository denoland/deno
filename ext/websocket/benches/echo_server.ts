// Copyright 2018-2026 the Deno authors. MIT license.
//
// Minimal Deno-native WebSocket echo server used by the fast-TCP path
// benchmark (issue #34xxx). Listens on FWS_ADDR (default 127.0.0.1:8080),
// echoes every text/binary frame back unchanged. No allocations beyond
// what the public WebSocket API forces; the optimization being measured
// is in ext/websocket itself.

const addr = Deno.env.get("FWS_ADDR") ?? "127.0.0.1:8080";
const [hostname, portStr] = addr.split(":");
const port = parseInt(portStr, 10);

Deno.serve({ hostname, port }, (req) => {
  if (req.headers.get("upgrade") !== "websocket") {
    return new Response("websocket only");
  }
  const { socket, response } = Deno.upgradeWebSocket(req);
  socket.binaryType = "arraybuffer";
  socket.onmessage = (e) => {
    try {
      socket.send(e.data);
    } catch (_) {
      /* socket closed under us; ignore */
    }
  };
  return response;
});
