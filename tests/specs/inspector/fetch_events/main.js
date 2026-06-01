// Covers two pieces of the fetch inspector instrumentation that the
// upstream node_compat test doesn't exercise:
//
//   1. Redirect chain - a 302 hop should keep the same requestId and the
//      next `requestWillBeSent` should carry a `redirectResponse` of the
//      previous hop. The intermediate 30x should NOT fire its own
//      `responseReceived`.
//   2. Streaming request body - when fetch's body is a ReadableStream,
//      `Network.getRequestPostData` must reject with "not finished yet"
//      because we never emit `dataSent({finished:true})` for that path.
import inspector from "node:inspector/promises";
import { strict as assert } from "node:assert";

const session = new inspector.Session();
session.connect();
await session.post("Network.enable");

// ---- Test 1: redirect chain ------------------------------------------------
{
  const server = Deno.serve({ port: 0, onListen: () => {} }, (req) => {
    const url = new URL(req.url);
    if (url.pathname === "/start") {
      return new Response(null, {
        status: 302,
        headers: { location: "/landing" },
      });
    }
    return new Response("ok", { headers: { "content-type": "text/plain" } });
  });

  const events = [];
  const onEvent = ({ method, params }) => events.push({ method, params });
  session.on("Network.requestWillBeSent", onEvent);
  session.on("Network.responseReceived", onEvent);
  session.on("Network.loadingFinished", onEvent);

  const startUrl = `http://127.0.0.1:${server.addr.port}/start`;
  const finalUrl = `http://127.0.0.1:${server.addr.port}/landing`;
  const resp = await fetch(startUrl);
  await resp.text();
  // Let the background drain emit loadingFinished.
  await new Promise((r) => setTimeout(r, 50));

  session.off("Network.requestWillBeSent", onEvent);
  session.off("Network.responseReceived", onEvent);
  session.off("Network.loadingFinished", onEvent);
  await server.shutdown();

  const willBeSent = events.filter((e) =>
    e.method === "Network.requestWillBeSent"
  );
  const received = events.filter((e) =>
    e.method === "Network.responseReceived"
  );
  const finished = events.filter((e) => e.method === "Network.loadingFinished");

  assert.equal(willBeSent.length, 2, "expected two requestWillBeSent events");
  assert.equal(
    willBeSent[0].params.requestId,
    willBeSent[1].params.requestId,
    "redirect hop should reuse the original requestId",
  );
  assert.equal(willBeSent[0].params.request.url, startUrl);
  assert.equal(willBeSent[0].params.redirectResponse, undefined);
  assert.equal(willBeSent[1].params.request.url, finalUrl);
  assert.equal(willBeSent[1].params.redirectResponse.status, 302);
  assert.equal(willBeSent[1].params.redirectResponse.url, startUrl);

  // The intermediate 302 must not produce its own responseReceived; only
  // the final 200 should.
  assert.equal(received.length, 1, "exactly one responseReceived");
  assert.equal(received[0].params.response.status, 200);
  assert.equal(received[0].params.response.url, finalUrl);

  assert.equal(finished.length, 1, "exactly one loadingFinished");
  assert.equal(
    finished[0].params.requestId,
    willBeSent[0].params.requestId,
    "loadingFinished should share the chain's requestId",
  );
  console.log("PASS: redirect chain emits with shared requestId");
}

// ---- Test 2: streaming request body ---------------------------------------
{
  const server = Deno.serve(
    { port: 0, onListen: () => {} },
    async (req) => new Response(await req.text()),
  );

  let requestId;
  const gotRequestWillBeSent = new Promise((resolve) => {
    session.once("Network.requestWillBeSent", ({ params }) => {
      requestId = params.requestId;
      resolve();
    });
  });

  const body = new ReadableStream({
    start(c) {
      c.enqueue(new TextEncoder().encode("streamed-body"));
      c.close();
    },
  });
  const resp = await fetch(`http://127.0.0.1:${server.addr.port}/`, {
    method: "POST",
    body,
  });
  await resp.text();
  await gotRequestWillBeSent;

  // For streaming bodies we deliberately don't flip is_request_finished -
  // there's no chunked dataSent path wired yet, so getRequestPostData
  // must reject rather than return garbage.
  let rejected = false;
  try {
    await session.post("Network.getRequestPostData", { requestId });
  } catch (err) {
    rejected = true;
    assert.match(
      String(err.message ?? err),
      /not finished yet/i,
      "should reject with 'not finished yet'",
    );
  }
  assert.equal(
    rejected,
    true,
    "getRequestPostData should reject for streaming bodies",
  );

  await server.shutdown();
  console.log("PASS: streaming-body request stays un-finished");
}

session.disconnect();
console.log("ALL PASSED");
