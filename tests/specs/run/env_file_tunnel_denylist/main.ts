// The denylisted Deno control variables must not have been imported from the
// env file...
console.log("DENO_CONNECTED:", Deno.env.get("DENO_CONNECTED"));
console.log(
  "DENO_DEPLOY_TUNNEL_ENDPOINT:",
  Deno.env.get("DENO_DEPLOY_TUNNEL_ENDPOINT"),
);
console.log(
  "DENO_EGRESS_HEADER_POLICY:",
  Deno.env.get("DENO_EGRESS_HEADER_POLICY"),
);
// ...but ordinary variables from the same file still are.
console.log("NOT_DENYLISTED:", Deno.env.get("NOT_DENYLISTED"));
console.log("user code started");

// The policy in the env file is deliberately malformed. Had the denylist let
// it through, the policy would be poisoned and this fetch would fail closed
// with the parse error rather than reaching the server, so a regression shows
// up here as well as in the `undefined` above.
const server = Deno.serve({ port: 0, onListen() {} }, () => new Response("ok"));
const response = await fetch(`http://localhost:${server.addr.port}/`);
console.log("fetch:", await response.text());
await server.shutdown();
