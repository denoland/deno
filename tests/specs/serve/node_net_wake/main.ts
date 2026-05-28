import net from "node:net";

const httpPort = 12477;
const tcpPort = 12478;

const listener = Deno.listen({ hostname: "127.0.0.1", port: tcpPort });

(async () => {
  try {
    for await (const conn of listener) {
      (async () => {
        const buf = new Uint8Array(16);
        await conn.read(buf);
        await conn.write(new TextEncoder().encode("ok"));
        conn.close();
      })();
    }
  } catch {
    // Listener shutdown.
  }
})();

const server = Deno.serve({
  hostname: "127.0.0.1",
  port: httpPort,
  onListen() {},
}, async () => {
  const body = await new Promise<string>((resolve, reject) => {
    const socket = net.connect(tcpPort, "127.0.0.1");
    socket.on("connect", () => socket.write("x"));
    socket.on("data", (chunk) => {
      resolve(String(chunk));
      socket.destroy();
    });
    socket.on("error", reject);
  });
  return new Response(body);
});

(async () => {
  try {
    const response = await fetch(`http://127.0.0.1:${httpPort}/`, {
      signal: AbortSignal.timeout(5000),
    });
    console.log(await response.text());
    await server.shutdown();
    listener.close();
    Deno.exit(0);
  } catch (error) {
    console.error(error);
    await server.shutdown().catch(() => {});
    listener.close();
    Deno.exit(2);
  }
})();
