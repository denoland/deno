const VERSION = 2;

const server = new Deno.QuicEndpoint({
  hostname: "localhost",
  port: 0,
});

const listener = server.listen({
  cert: Deno.readTextFileSync("../../../testdata/tls/localhost.crt"),
  key: Deno.readTextFileSync("../../../testdata/tls/localhost.key"),
  alpnProtocols: ["🦕🕳️"],
});

const child = new Deno.Command(Deno.execPath(), {
  cwd: Deno.cwd(),
  args: [
    "run",
    "-A",
    "--connected",
    "--cert",
    "../../../testdata/tls/RootCA.crt",
    "client.ts",
  ],
  env: {
    DENO_DEPLOY_TUNNEL_ENDPOINT: `localhost:${server.addr.port}`,
    DENO_DEPLOY_TOKEN: "token",
    DENO_DEPLOY_ORG: "org",
    DENO_DEPLOY_APP: "app",
  },
  stdout: "inherit",
  stderr: "inherit",
}).spawn();

setTimeout(() => {
  console.error("timeout, killing");
  child.kill("SIGKILL");
  Deno.exit(1);
}, 5000);

for await (const incoming of listener) {
  handleConnection(incoming);
}

async function handleConnection(incoming: Deno.QuicIncoming) {
  const conn = await incoming.accept();

  {
    const { value: bi } = await conn.incomingBidirectionalStreams
      .getReader()
      .read();

    const reader = bi.readable.getReader({ mode: "byob" });
    const version = await readUint32LE(reader);
    if (version !== VERSION) {
      conn.close({ closeCode: 1, reason: "invalid version" });
      return;
    }
    const writer = bi.writable.getWriter();
    await writeUint32LE(writer, VERSION);
    const header = await readStreamHeader(reader);
    if (header.headerType !== "Control") {
      conn.close({ closeCode: 1, reason: "expected Control" });
      return;
    }
    console.log({
      subcommand: header.metadata.subcommand,
      entrypoint: header.metadata.entrypoint,
    });
    const auth = await readStreamHeader(reader);
    if (auth.headerType !== "AuthenticateApp") {
      conn.close({ closeCode: 1, reason: "expected AuthenticateApp" });
      return;
    }
    await writeStreamHeader(writer, {
      headerType: "Authenticated",
      hostnames: ["localhost"],
      addr: `${
        server.addr.hostname.includes(":")
          ? `[${server.addr.hostname}]`
          : server.addr.hostname
      }:${server.addr.port}`,
      env: {},
      metadata: {},
    });

    const next = await readStreamHeader(reader);
    if (next.headerType !== "Listening") {
      conn.close({ closeCode: 1, reason: "expected Listening" });
      return;
    }

    await writeStreamHeader(writer, {
      headerType: "Routed",
    });

    reader.releaseLock();
    writer.releaseLock();
  }

  {
    const stream = await conn.createBidirectionalStream();
    const writer = stream.writable.getWriter();
    await writeStreamHeader(writer, {
      headerType: "Stream",
      local_addr: "1.2.3.4:80",
      remote_addr: "1.2.3.4:80",
    });

    await writer.write(
      new TextEncoder().encode(`GET / HTTP/1.1\r\nHost: localhost\r\n\r\n`),
    );

    const reader = stream.readable.getReader({ mode: "byob" });
    const { value } = await reader.read(new Uint8Array(1024));
    console.log(new TextDecoder().decode(value));

    child.kill("SIGKILL");
    Deno.exit(0);
  }
}

async function readUint32LE(reader) {
  const { value: view } = await reader.read(new Uint8Array(4), { min: 4 });
  return new DataView(view.buffer).getUint32(0, true);
}

async function writeUint32LE(writer, value) {
  const view = new Uint8Array(4);
  new DataView(view.buffer).setUint32(0, value, true);
  await writer.write(view);
}

async function readStreamHeader(reader) {
  const length = await readUint32LE(reader);
  const { value: view } = await reader.read(new Uint8Array(length), {
    min: length,
  });
  const data = JSON.parse(new TextDecoder().decode(view));
  const items = Object.entries(data);
  if (items.length !== 1) {
    throw new Error("invalid header");
  }
  items[0][1].headerType = items[0][0];
  return items[0][1];
}

async function writeStreamHeader(writer, header) {
  const { headerType, ...headerData } = header;
  const data = { [headerType]: headerData };
  const view = new TextEncoder().encode(JSON.stringify(data));
  await writeUint32LE(writer, view.length);
  await writer.write(view);
}
