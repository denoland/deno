// EventSource dispatches through the fetch stack without going through
// `fetch()` itself, so it is the regression test for the policy being applied
// at the shared entry point: its request must carry the same policy-owned
// headers a plain fetch() would, and must forward the inbound request's
// values when it is opened from inside a serve handler.

const seen: string[] = [];

const server = Deno.serve({ port: 0, onListen() {} }, (req: Request) => {
  const url = new URL(req.url);
  if (url.pathname === "/events") {
    seen.push(
      `${url.searchParams.get("from")}: ${req.headers.get("cdn-loop")}`,
    );
    return new Response("", {
      headers: { "content-type": "text/event-stream" },
    });
  }
  // Opened from inside a handler, so the inbound cdn-loop is in scope and
  // must be forwarded onto the EventSource request.
  return openEventSource("handler").then(() => new Response("ok"));
});

const base = `http://localhost:${server.addr.port}`;

function openEventSource(from: string): Promise<void> {
  return new Promise((resolve) => {
    const es = new EventSource(`${base}/events?from=${from}`);
    const done = () => {
      es.close();
      resolve();
    };
    es.onopen = done;
    es.onerror = done;
  });
}

// Top level: no inbound request in scope, so only the appended entry.
await openEventSource("top-level");

// Through a serve hop: the handler's inbound entry is forwarded, then the
// policy's own entry is appended.
await (await fetch(`${base}/proxy`)).text();

for (const line of seen) console.log(line);

await server.shutdown();
