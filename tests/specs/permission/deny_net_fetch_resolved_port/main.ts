// Regression test: --deny-net=127.0.0.1:<port> must block fetch() requests
// whose hostname resolves to the denied (ip, port) pair. The check needs to
// run with the real request port, not the placeholder 0 that hyper-util's
// DNS service receives.

const server = Deno.serve(
  { hostname: "0.0.0.0", port: 0, onListen: () => {} },
  () => new Response("hello"),
);
const denyPort = (server.addr as Deno.NetAddr).port;

// A second server on a different port so we can prove the deny rule is
// port-scoped — fetching this one through `localhost` must still succeed.
const allowedServer = Deno.serve(
  { hostname: "0.0.0.0", port: 0, onListen: () => {} },
  () => new Response("allowed"),
);
const allowedPort = (allowedServer.addr as Deno.NetAddr).port;

const script = `
try {
  await fetch("http://127.0.0.1:${denyPort}/");
  console.log("FAIL: fetch to 127.0.0.1 (deny port) was not denied");
} catch {
  console.log("PASS: fetch to 127.0.0.1 (deny port) denied");
}

try {
  await fetch("http://localhost:${denyPort}/");
  console.log("FAIL: fetch to localhost (deny port via DNS) was not denied");
} catch {
  console.log("PASS: fetch to localhost (deny port via DNS) denied");
}

try {
  const res = await fetch("http://localhost:${allowedPort}/");
  await res.text();
  console.log("PASS: fetch to localhost (different port) allowed");
} catch (e) {
  console.log("FAIL: fetch to localhost (different port):", String(e));
}
`;

const child = new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--allow-net",
    `--deny-net=127.0.0.1:${denyPort},[::1]:${denyPort}`,
    "-",
  ],
  stdin: "piped",
  stdout: "piped",
  stderr: "inherit",
}).spawn();
const writer = child.stdin.getWriter();
await writer.write(new TextEncoder().encode(script));
await writer.close();

const { stdout } = await child.output();
await Deno.stdout.write(stdout);

await server.shutdown();
await allowedServer.shutdown();
