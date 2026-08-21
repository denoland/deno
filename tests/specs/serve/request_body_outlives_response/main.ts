// Regression test for https://github.com/denoland/deno/issues/36624
//
// The request body stream must remain readable to end-of-stream regardless of
// the response lifecycle. Four facets are covered:
//
//   1. The handler crosses a macrotask boundary, then returns a streaming
//      response held open until `req.body` finishes piping. The final request
//      body chunks and EOS must still be delivered (they were stalling).
//   2. The handler returns a complete (bodyless) response while `req.body` is
//      still being read in the background. The read must finish instead of
//      failing with "underlying resource unavailable".
//   3. Same as 2, but the response takes the fast static path (a plain string
//      body), which is written without going through the record.
//   4. The handler reads part of `req.body` and then abandons the reader. The
//      connection must not be pinned by a reader that will never finish, so
//      `server.shutdown()` still resolves.

const BODY = "twelve bytes";

function makeClientBody(): {
  body: ReadableStream<Uint8Array>;
  close: () => void;
} {
  let close!: () => void;
  const body = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(new TextEncoder().encode(BODY));
      close = () => controller.close();
    },
  });
  return { body, close };
}

// Counts the bytes piped into it, so a truncated body is caught as well as a
// stalled one.
class CountingSink {
  read = 0;
  stream = new WritableStream<Uint8Array>({
    write: (chunk) => {
      this.read += chunk.byteLength;
    },
  });
}

function sendBody(port: number): {
  response: Promise<Response>;
  finishBody: () => void;
} {
  const { body, close } = makeClientBody();
  const response = fetch(`http://127.0.0.1:${port}/x`, {
    method: "PATCH",
    body,
  });
  return {
    response,
    finishBody: () => close(),
  };
}

// Facet 1: streaming response held open while the request body is piped.
async function facetHeldOpenStreamingResponse() {
  const sink = new CountingSink();
  const server = Deno.serve({ port: 0, onListen: () => {} }, async (req) => {
    // Any macrotask boundary before responding used to trigger the stall.
    await new Promise((r) => setTimeout(r, 0));
    if (req.method !== "PATCH" || !req.body) {
      return new Response(null, { status: 400 });
    }
    const pipe = req.body.pipeTo(sink.stream);
    return new Response(
      new ReadableStream({
        async start(controller) {
          await pipe;
          controller.close();
        },
      }),
      { status: 201 },
    );
  });

  const { response, finishBody } = sendBody(server.addr.port);
  await new Promise((r) => setTimeout(r, 200));
  finishBody();

  const res = await response;
  console.log("facet1 status:", res.status);
  // If the request body stalled, this pipe never resolves and the test hangs.
  await res.body?.pipeTo(new WritableStream());
  console.log("facet1: response body drained,", sink.read, "bytes read");
  await server.shutdown();
}

// Facet 2: complete response while the body pipe continues in the background.
async function facetCompletedResponse() {
  const pipeResult = Promise.withResolvers<string>();
  const sink = new CountingSink();
  const server = Deno.serve({ port: 0, onListen: () => {} }, (req) => {
    if (req.method !== "PATCH" || !req.body) {
      return new Response(null, { status: 400 });
    }
    req.body.pipeTo(sink.stream).then(
      () => pipeResult.resolve(`done, ${sink.read} bytes read`),
      (err) => pipeResult.resolve(`error: ${err?.message ?? err}`),
    );
    return new Response(null, { status: 201 });
  });

  const { response, finishBody } = sendBody(server.addr.port);
  await new Promise((r) => setTimeout(r, 200));
  finishBody();

  const res = await response;
  console.log("facet2 status:", res.status);
  await res.body?.pipeTo(new WritableStream());
  console.log("facet2 pipe:", await pipeResult.promise);
  await server.shutdown();
}

// Facet 3: as facet 2, but with a fast-path static response body.
async function facetFastStaticResponse() {
  const pipeResult = Promise.withResolvers<string>();
  const sink = new CountingSink();
  const server = Deno.serve({ port: 0, onListen: () => {} }, (req) => {
    if (req.method !== "PATCH" || !req.body) {
      return new Response(null, { status: 400 });
    }
    req.body.pipeTo(sink.stream).then(
      () => pipeResult.resolve(`done, ${sink.read} bytes read`),
      (err) => pipeResult.resolve(`error: ${err?.message ?? err}`),
    );
    return new Response("ok");
  });

  const { response, finishBody } = sendBody(server.addr.port);
  await new Promise((r) => setTimeout(r, 200));
  finishBody();

  const res = await response;
  console.log("facet3 status:", res.status, "body:", await res.text());
  console.log("facet3 pipe:", await pipeResult.promise);
  await server.shutdown();
}

// Facet 4: the handler abandons the body reader after a partial read. The
// connection must not be held hostage by a reader that never finishes.
async function facetAbandonedReader() {
  const server = Deno.serve({ port: 0, onListen: () => {} }, async (req) => {
    if (req.method !== "PATCH" || !req.body) {
      return new Response(null, { status: 400 });
    }
    // One chunk, then walk away: no cancel, no releaseLock, no further reads.
    const reader = req.body.getReader();
    await reader.read();
    return new Response(null, { status: 201 });
  });

  const { response, finishBody } = sendBody(server.addr.port);
  await new Promise((r) => setTimeout(r, 200));
  finishBody();

  const res = await response;
  console.log("facet4 status:", res.status);
  await res.body?.cancel();
  // Hangs if the connection loop is still parked waiting for that reader.
  await server.shutdown();
  console.log("facet4: shutdown completed");
}

// Guard against a regression re-introducing the stall: fail fast instead of
// hanging the test suite.
function withTimeout<T>(p: Promise<T>, label: string): Promise<T> {
  let timer: number;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(
      () => reject(new Error(`${label} timed out (request body stalled?)`)),
      15_000,
    );
  });
  return Promise.race([p, timeout]).finally(() =>
    clearTimeout(timer!)
  ) as Promise<T>;
}

await withTimeout(facetHeldOpenStreamingResponse(), "facet1");
await withTimeout(facetCompletedResponse(), "facet2");
await withTimeout(facetFastStaticResponse(), "facet3");
await withTimeout(facetAbandonedReader(), "facet4");
console.log("PASS");
