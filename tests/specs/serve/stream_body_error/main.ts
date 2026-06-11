// Regression test for https://github.com/denoland/deno/issues/19867
//
// A `Deno.serve` response whose body is a `ReadableStream`/`TransformStream`
// that throws while being drained used to swallow the error completely: the
// client saw a truncated response and nothing on the server implicated the
// faulty callback. The error must now be surfaced through the server's error
// handler so a stack trace pointing at the throwing transformer is printed,
// and the server must stay alive for subsequent requests.

const ac = new AbortController();
const server = Deno.serve(
  { port: 0, signal: ac.signal, onListen() {} },
  (req) => {
    if (new URL(req.url).pathname === "/ok") {
      return new Response("second request ok");
    }
    return new Response(
      new ReadableStream({
        start(c) {
          c.enqueue(new TextEncoder().encode("test"));
          c.close();
        },
      })
        .pipeThrough(new TextDecoderStream())
        .pipeThrough(
          new TransformStream({
            transform(value, c) {
              // Intentional typo (`toUppperCase`) so the transformer throws.
              // @ts-ignore intentional typo
              c.enqueue(value.toUppperCase());
            },
          }),
        )
        .pipeThrough(new TextEncoderStream()),
    );
  },
);

const port = (server.addr as Deno.NetAddr).port;

const res = await fetch(`http://localhost:${port}/`);
console.log("status:", res.status);
try {
  await res.text();
  console.log("body read: ok");
} catch {
  console.log("body read: errored");
}

// Give the streaming error a moment to be reported on the server side.
await new Promise((r) => setTimeout(r, 100));

// The server must still be alive to serve a second request.
const res2 = await fetch(`http://localhost:${port}/ok`);
console.log("second status:", res2.status, "-", await res2.text());

ac.abort();
await server.finished;
console.log("server shut down cleanly");
