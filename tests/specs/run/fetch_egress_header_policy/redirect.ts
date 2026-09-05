// Redirect behaviour of the two application points.
//
// `append` runs once per fetch() call, so its entry must survive redirect hops
// without duplicating. `set`/`default` re-run per hop, so they must not undo
// the cross-origin credential strip that `httpRedirectFetch` performs.
//
// The policy here sets `authorization` because that is one of the headers the
// strip covers, which is the whole point of the test. It is not a suggestion:
// a policy lives in an env var that user code can read under --allow-env and
// that every subprocess inherits, so real credentials do not belong in one.
// See the scope notes in ext/fetch/egress_policy.rs.

// One server serves both roles, so a redirect to its own `localhost` URL is
// same-origin while a redirect to its `127.0.0.1` URL is not.
const server = Deno.serve({ port: 0, onListen() {} }, (req: Request) => {
  const url = new URL(req.url);
  if (url.pathname === "/redirect") {
    return Response.redirect(url.searchParams.get("to")!, 302);
  }
  return Response.json({
    "cdn-loop": req.headers.get("cdn-loop"),
    "authorization": req.headers.get("authorization"),
    "user-agent": req.headers.get("user-agent"),
  });
});

const port = server.addr.port;

async function through(target: string) {
  const resp = await fetch(
    `http://localhost:${port}/redirect?to=${encodeURIComponent(target)}`,
  );
  return await resp.json();
}

{
  // Same-origin redirect: the appended entry crosses the hop exactly once,
  // and the operator credential is still attached.
  const headers = await through(`http://localhost:${port}/echo`);
  console.log("same-origin cdn-loop:", headers["cdn-loop"]);
  console.log("same-origin authorization:", headers["authorization"]);
}

{
  // Cross-origin redirect: WHATWG fetch drops the credential and the policy
  // must not put it back. Non-credential ops still apply.
  const headers = await through(`http://127.0.0.1:${port}/echo`);
  console.log("cross-origin cdn-loop:", headers["cdn-loop"]);
  console.log("cross-origin authorization:", headers["authorization"]);
  console.log("cross-origin user-agent:", headers["user-agent"]);
}

await server.shutdown();
