const { serve, upgradeHttpRaw } = Deno;
const u8 = Deno[Deno.internal].core.encode(
  "HTTP/1.1 101 Switching Protocols\r\n\r\n",
);

async function handler(req) {
  const [conn, _firstPacket] = upgradeHttpRaw(req);
  await conn.write(u8);
  await conn.close();
}

serve(handler, { hostname: "127.0.0.1", port: 9000 });
