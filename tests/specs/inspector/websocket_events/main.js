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
//   3. Frame events + server-side instrumentation - send text+binary from
//      client, echo from server. Verify both sides emit webSocketCreated
//      and that frameSent/frameReceived fire with correct opcode, mask,
//      and payloadData on both client and server sockets.
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

  // The inspector sees both sides in this process: the `new WebSocket()`
  // (client) and the `Deno.upgradeWebSocket(req)` (server) each emit their
  // own `webSocketCreated` with distinct requestIds.
  assert.equal(created.length, 2, "client + server webSocketCreated");
  assert.equal(
    willSend.length,
    1,
    "exactly one webSocketWillSendHandshakeRequest (client only)",
  );
  assert.equal(
    handshake.length,
    1,
    "exactly one handshakeResponseReceived (client only)",
  );
  assert.ok(closed.length >= 1, "at least one webSocketClosed");

  // Client-side requestId is the one that matches willSend/handshake.
  const clientRequestId = willSend[0].params.requestId;
  assert.ok(clientRequestId, "requestId must be present");
  const clientCreated = created.find(
    (e) => e.params.requestId === clientRequestId,
  );
  assert.ok(clientCreated, "client webSocketCreated must match willSend");
  assert.equal(handshake[0].params.requestId, clientRequestId);
  assert.ok(
    closed.some((e) => e.params.requestId === clientRequestId),
    "client webSocketClosed must share requestId",
  );

  assert.equal(
    clientCreated.params.url,
    `ws://127.0.0.1:${server.addr.port}/`,
  );
  assert.equal(handshake[0].params.response.status, 101);
  assert.ok(
    handshake[0].params.response.headers,
    "response headers should be populated",
  );

  console.log("PASS: happy path emits client and server lifecycle events");
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

// ---- Test 3: frame events + server-side instrumentation ------------------
{
  const server = Deno.serve({ port: 0, onListen: () => {} }, (req) => {
    if (req.headers.get("upgrade") !== "websocket") {
      return new Response("not a ws request", { status: 400 });
    }
    const { socket, response } = Deno.upgradeWebSocket(req);
    socket.binaryType = "arraybuffer";
    socket.addEventListener("message", (e) => {
      // Echo text back as text, binary back as binary.
      socket.send(e.data);
    });
    return response;
  });

  const events = [];
  const onEvent = ({ method, params }) => events.push({ method, params });
  const methods = [
    "Network.webSocketCreated",
    "Network.webSocketFrameSent",
    "Network.webSocketFrameReceived",
    "Network.webSocketClosed",
  ];
  for (const m of methods) session.on(m, onEvent);

  const ws = new WebSocket(`ws://127.0.0.1:${server.addr.port}/`);
  ws.binaryType = "arraybuffer";
  await waitForOpen(ws);

  const textReply = new Promise((r) =>
    ws.addEventListener("message", (e) => r(e.data), { once: true })
  );
  ws.send("hello-ws");
  assert.equal(await textReply, "hello-ws", "text echo");

  const binReply = new Promise((r) =>
    ws.addEventListener("message", (e) => r(e.data), { once: true })
  );
  ws.send(new Uint8Array([0xde, 0xad, 0xbe, 0xef]));
  const bin = new Uint8Array(await binReply);
  assert.deepEqual(
    Array.from(bin),
    [0xde, 0xad, 0xbe, 0xef],
    "binary echo round-trips",
  );

  ws.close(1000, "bye");
  await waitForClose(ws);
  await new Promise((r) => setTimeout(r, 50));
  for (const m of methods) session.off(m, onEvent);
  await server.shutdown();

  const created = events.filter((e) => e.method === "Network.webSocketCreated");
  const sent = events.filter((e) => e.method === "Network.webSocketFrameSent");
  const received = events.filter((e) =>
    e.method === "Network.webSocketFrameReceived"
  );
  const closed = events.filter((e) => e.method === "Network.webSocketClosed");

  // Two webSocketCreated: one from the client `new WebSocket`, one from
  // the server-side Deno.upgradeWebSocket. Both in the same process so the
  // inspector sees both.
  assert.equal(created.length, 2, "client + server both emit webSocketCreated");
  const requestIds = new Set(created.map((e) => e.params.requestId));
  assert.equal(requestIds.size, 2, "client and server use distinct requestIds");

  // Two frames each direction × two sockets = 4 sent + 4 received total.
  // (Client sends text+binary, server echoes both → server also sends two
  // and receives two.)
  assert.equal(sent.length, 4, "4 frameSent (2 client + 2 server)");
  assert.equal(received.length, 4, "4 frameReceived (2 client + 2 server)");
  // Both sockets eventually emit webSocketClosed.
  assert.ok(closed.length >= 1, "at least one webSocketClosed");

  // Opcodes: equal counts of text (1) and binary (2) per direction.
  const opSent = sent.map((e) => e.params.response.opcode).sort();
  const opRecv = received.map((e) => e.params.response.opcode).sort();
  assert.deepEqual(opSent, [1, 1, 2, 2], "opcodes 1,1,2,2 sent");
  assert.deepEqual(opRecv, [1, 1, 2, 2], "opcodes 1,1,2,2 received");

  // RFC 6455: client masks outgoing, server doesn't. The inspector sees
  // both sockets in this process so each direction is represented twice.
  const masksSent = sent.map((e) => e.params.response.mask).sort();
  const masksRecv = received.map((e) => e.params.response.mask).sort();
  // 2 client-sent (mask=true) + 2 server-sent (mask=false) = [false,false,true,true]
  assert.deepEqual(masksSent, [false, false, true, true], "mask flags on sent");
  assert.deepEqual(masksRecv, [false, false, true, true], "mask flags on recv");

  // payloadData: text frames pass through as the string; binary frames are
  // base64-encoded by the bridge.
  const textSent = sent.filter((e) => e.params.response.opcode === 1);
  for (const e of textSent) {
    assert.equal(e.params.response.payloadData, "hello-ws");
  }
  const binSent = sent.filter((e) => e.params.response.opcode === 2);
  for (const e of binSent) {
    // base64 of <DE AD BE EF> is "3q2+7w==".
    assert.equal(e.params.response.payloadData, "3q2+7w==");
  }

  console.log("PASS: frame events fire for client and server sockets");
}

session.disconnect();
console.log("ALL PASSED");
