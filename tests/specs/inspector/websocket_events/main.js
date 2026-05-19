// Covers the WebSocket inspector instrumentation that the upstream
// node_compat test can't exercise (it asserts an "undici" frame in the
// initiator stack, which Deno's native WebSocket doesn't produce).
//
//   1. Happy path - server accepts the upgrade, client closes cleanly.
//      Expect: webSocketCreated -> webSocketWillSendHandshakeRequest ->
//      webSocketHandshakeResponseReceived -> webSocketClosed, all sharing
//      one requestId.
//   2. Error path - server 404s the upgrade. Expect: webSocketCreated ->
//      webSocketWillSendHandshakeRequest -> webSocketClosed (no
//      handshakeResponseReceived, since non-101 bails before the surface).
import inspector from "node:inspector/promises";
import { strict as assert } from "node:assert";

const session = new inspector.Session();
session.connect();
await session.post("Network.enable");

function collectWebSocketEvents() {
  const events = [];
  const onEvent = ({ method, params }) => events.push({ method, params });
  const methods = [
    "Network.webSocketCreated",
    "Network.webSocketWillSendHandshakeRequest",
    "Network.webSocketHandshakeResponseReceived",
    "Network.webSocketClosed",
  ];
  for (const m of methods) session.on(m, onEvent);
  return {
    events,
    stop() {
      for (const m of methods) session.off(m, onEvent);
    },
  };
}

function waitForOpen(ws) {
  return new Promise((resolve, reject) => {
    ws.addEventListener("open", () => resolve(), { once: true });
    ws.addEventListener("error", (e) => reject(e.error ?? new Error("error")), {
      once: true,
    });
  });
}

function waitForClose(ws) {
  return new Promise((resolve) => {
    ws.addEventListener("close", () => resolve(), { once: true });
  });
}

// ---- Test 1: happy path ---------------------------------------------------
{
  const server = Deno.serve({ port: 0, onListen: () => {} }, (req) => {
    if (req.headers.get("upgrade") !== "websocket") {
      return new Response("not a ws request", { status: 400 });
    }
    const { socket, response } = Deno.upgradeWebSocket(req);
    socket.addEventListener("message", (e) => socket.send(e.data));
    return response;
  });

  const { events, stop } = collectWebSocketEvents();
  const ws = new WebSocket(`ws://127.0.0.1:${server.addr.port}/`);
  await waitForOpen(ws);
  ws.close(1000, "bye");
  await waitForClose(ws);
  // Let pending inspector messages flush onto the session.
  await new Promise((r) => setTimeout(r, 50));
  stop();
  await server.shutdown();

  const created = events.filter((e) => e.method === "Network.webSocketCreated");
  const willSend = events.filter((e) =>
    e.method === "Network.webSocketWillSendHandshakeRequest"
  );
  const handshake = events.filter((e) =>
    e.method === "Network.webSocketHandshakeResponseReceived"
  );
  const closed = events.filter((e) => e.method === "Network.webSocketClosed");

  assert.equal(created.length, 1, "exactly one webSocketCreated");
  assert.equal(
    willSend.length,
    1,
    "exactly one webSocketWillSendHandshakeRequest",
  );
  assert.equal(handshake.length, 1, "exactly one handshakeResponseReceived");
  assert.equal(closed.length, 1, "exactly one webSocketClosed");

  const requestId = created[0].params.requestId;
  assert.ok(requestId, "requestId must be present");
  assert.equal(willSend[0].params.requestId, requestId);
  assert.equal(handshake[0].params.requestId, requestId);
  assert.equal(closed[0].params.requestId, requestId);

  assert.equal(created[0].params.url, `ws://127.0.0.1:${server.addr.port}/`);
  assert.equal(handshake[0].params.response.status, 101);
  assert.ok(
    handshake[0].params.response.headers,
    "response headers should be populated",
  );

  console.log("PASS: happy path emits four events with shared requestId");
}

// ---- Test 2: failed handshake ---------------------------------------------
{
  const server = Deno.serve(
    { port: 0, onListen: () => {} },
    () => new Response("nope", { status: 404 }),
  );

  const { events, stop } = collectWebSocketEvents();
  const ws = new WebSocket(`ws://127.0.0.1:${server.addr.port}/`);
  // Add both listeners up front - error and close fire back-to-back on the
  // failure path, so registering close after awaiting error would miss it.
  const closePromise = waitForClose(ws);
  await new Promise((resolve) => {
    ws.addEventListener("error", () => resolve(), { once: true });
  });
  await closePromise;
  await new Promise((r) => setTimeout(r, 50));
  stop();
  await server.shutdown();

  const created = events.filter((e) => e.method === "Network.webSocketCreated");
  const willSend = events.filter((e) =>
    e.method === "Network.webSocketWillSendHandshakeRequest"
  );
  const handshake = events.filter((e) =>
    e.method === "Network.webSocketHandshakeResponseReceived"
  );
  const closed = events.filter((e) => e.method === "Network.webSocketClosed");

  assert.equal(created.length, 1, "expected webSocketCreated on failure");
  assert.equal(
    willSend.length,
    1,
    "expected willSendHandshakeRequest on failure",
  );
  assert.equal(
    handshake.length,
    0,
    "no handshakeResponseReceived for non-101 (bails in Rust)",
  );
  assert.equal(closed.length, 1, "expected webSocketClosed on failure");
  assert.equal(
    created[0].params.requestId,
    closed[0].params.requestId,
    "failure path should share requestId",
  );

  console.log("PASS: failed handshake emits created + willSend + closed only");
}

session.disconnect();
console.log("ALL PASSED");
