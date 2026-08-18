// Regression test for https://github.com/denoland/deno/issues/36624
//
// The request body stream must remain readable to end-of-stream regardless of
// the response lifecycle. Two facets are covered:
//
//   1. The handler crosses a macrotask boundary, then returns a streaming
//      response held open until `req.body` finishes piping. The final request
//      body chunks and EOS must still be delivered (they were stalling).
//   2. The handler returns a complete (bodyless) response while `req.body` is
//      still being read in the background. The read must finish instead of
//      failing with "underlying resource unavailable".

function makeClientBody(): {
  body: ReadableStream<Uint8Array>;
  close: () => void;
} {
  let close!: () => void;
  const body = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(new TextEncoder().encode("twelve bytes"));
      close = () => controller.close();
    },
  });
  return { body, close };
}

// Facet 1: streaming response held open while the request body is piped.
async function facetHeldOpenStreamingResponse() {
  const server = Deno.serve({ port: 0, onListen: () => {} }, async (req) => {
    // Any macrotask boundary before responding used to trigger the stall.
    await new Promise((r) => setTimeout(r, 0));
    if (req.method !== "PATCH" || !req.body) {
      return new Response(null, { status: 400 });
    }
    const pipe = req.body.pipeTo(new WritableStream());
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

  const { body, close } = makeClientBody();
  const patchPromise = fetch(`http://127.0.0.1:${server.addr.port}/x`, {
    method: "PATCH",
    body,
  });
  await new Promise((r) => setTimeout(r, 200));
  close();

  const res = await patchPromise;
  console.log("facet1 status:", res.status);
  // If the request body stalled, this pipe never resolves and the test hangs.
  await res.body?.pipeTo(new WritableStream());
  console.log("facet1: response body drained");
  await server.shutdown();
}

// Facet 2: complete response while the body pipe continues in the background.
async function facetCompletedResponse() {
  const pipeResult = Promise.withResolvers<string>();
  const server = Deno.serve({ port: 0, onListen: () => {} }, (req) => {
    if (req.method !== "PATCH" || !req.body) {
      return new Response(null, { status: 400 });
    }
    req.body.pipeTo(new WritableStream()).then(
      () => pipeResult.resolve("done"),
      (err) => pipeResult.resolve(`error: ${err?.message ?? err}`),
    );
    return new Response(null, { status: 201 });
  });

  const { body, close } = makeClientBody();
  const patchPromise = fetch(`http://127.0.0.1:${server.addr.port}/x`, {
    method: "PATCH",
    body,
  });
  await new Promise((r) => setTimeout(r, 200));
  close();

  const res = await patchPromise;
  console.log("facet2 status:", res.status);
  await res.body?.pipeTo(new WritableStream());
  console.log("facet2 pipe:", await pipeResult.promise);
  await server.shutdown();
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
console.log("PASS");
