// Regression test for https://github.com/denoland/deno/issues/36982
//
// Aborting a fetch must cancel the response body even after the Response
// has been collected. The body cancellation is an abort algorithm on the
// request's dependent signal, which the source only held through a WeakRef.

declare const gc: (opts?: object) => void;

const server = Deno.serve({ port: 0, onListen() {} }, () =>
  new Response(
    new ReadableStream({
      start(controller) {
        controller.enqueue(new TextEncoder().encode("first chunk"));
      },
    }),
  ));

async function openBody(signal: AbortSignal) {
  const res = await fetch(`http://127.0.0.1:${server.addr.port}/`, { signal });
  return res.body!.getReader();
}

async function abortAfter(collect: boolean) {
  const controller = new AbortController();
  const reader = await openBody(controller.signal);
  await reader.read();
  if (collect) {
    // The Response is already out of openBody. GC from a fresh macrotask so
    // the stack frame that created it is gone.
    await new Promise((resolve) => setTimeout(resolve, 0));
    gc({ type: "major", execution: "sync" });
  }
  controller.abort();
  const outcome = await Promise.race([
    reader.read().then(() => "resolved", (e: Error) => `rejected: ${e.name}`),
    new Promise((resolve) =>
      setTimeout(() => resolve("still pending after 2s"), 2000)
    ),
  ]);
  console.log(`gc before abort: ${collect} -> ${outcome}`);
  try {
    await reader.cancel();
  } catch {
    // The abort already errored the stream.
  }
}

await abortAfter(false);
await abortAfter(true);
await server.shutdown();
