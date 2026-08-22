// Inbound header names arrive with the client's on-the-wire casing on the
// HTTP/1.1 server path, while the policy's `forward` names are always
// lowercase. Matching must be case-insensitive, or forwarding silently
// captures nothing for every client that uses the canonical spelling.

const echo = Deno.serve({ port: 0, onListen() {} }, (req: Request) => {
  return Response.json({ "cdn-loop": req.headers.get("cdn-loop") });
});

const proxy = Deno.serve({ port: 0, onListen() {} }, async (_req: Request) => {
  const resp = await fetch(`http://localhost:${echo.addr.port}/`);
  return Response.json(await resp.json());
});

// A hand-written request, so the inbound casing is ours rather than the one
// Deno's own fetch would produce.
async function request(headerLine: string): Promise<string> {
  const conn = await Deno.connect({
    hostname: "127.0.0.1",
    port: proxy.addr.port,
  });
  await conn.write(
    new TextEncoder().encode(
      `GET / HTTP/1.1\r\nHost: localhost\r\n${headerLine}\r\nConnection: close\r\n\r\n`,
    ),
  );
  const chunks: string[] = [];
  const buf = new Uint8Array(4096);
  while (true) {
    const n = await conn.read(buf);
    if (n === null) break;
    chunks.push(new TextDecoder().decode(buf.subarray(0, n)));
  }
  const raw = chunks.join("");
  return JSON.parse(raw.slice(raw.indexOf("\r\n\r\n") + 4))["cdn-loop"];
}

// Printing the value rather than a comparison keeps a regression readable:
// dropping the forwarded entry shows up as the appended entry alone.
for (const spelling of ["cdn-loop", "CDN-Loop", "Cdn-LOOP"]) {
  console.log(`${spelling}:`, await request(`${spelling}: outside;d=1`));
}

await echo.shutdown();
await proxy.shutdown();
