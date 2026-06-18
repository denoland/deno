// Regression test: --deny-net=127.0.0.1 must block fetch() requests whose
// hostname resolves to a denied IP, mirroring the post-resolution check that
// Deno.connect already performs.

const server = Deno.serve(
  { hostname: "0.0.0.0", port: 0, onListen: () => {} },
  () => new Response("hello"),
);
const { port } = server.addr as Deno.NetAddr;

try {
  await fetch(`http://127.0.0.1:${port}/`);
  console.log("FAIL: fetch to 127.0.0.1 was not denied");
} catch {
  console.log("PASS: fetch to 127.0.0.1 denied");
}

try {
  await fetch(`http://localhost:${port}/`);
  console.log(
    "FAIL: fetch to localhost (resolves to denied IP) was not denied",
  );
} catch {
  console.log("PASS: fetch to localhost denied");
}

await server.shutdown();

// The denial must happen before any socket is opened: a denied fetch() must
// never complete a TCP handshake to the forbidden IP (matching Deno.connect,
// which denies before connecting). Use a raw listener so even a bare
// handshake with no request bytes is detected. Bind to 0.0.0.0 (not denied)
// so the listener itself is permitted; connections to 127.0.0.1 still land
// on it.
const listener = Deno.listen({ hostname: "0.0.0.0", port: 0 });
let handshakeCompleted = false;
const accepted = listener.accept().then(
  (conn) => {
    handshakeCompleted = true;
    conn.close();
  },
  () => {}, // rejects with BadResource when the listener closes
);

try {
  await fetch(`http://localhost:${listener.addr.port}/`);
  console.log("FAIL: fetch to localhost was not denied");
} catch {
  console.log("PASS: fetch to localhost denied");
}

// Give a stray connection a chance to surface before checking.
await new Promise((resolve) => setTimeout(resolve, 100));
listener.close();
await accepted;
if (handshakeCompleted) {
  console.log("FAIL: TCP handshake to denied IP completed before denial");
} else {
  console.log("PASS: no socket was opened to the denied IP");
}
