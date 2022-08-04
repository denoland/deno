const { serve, upgradeHttp } = Deno.flash;
const u8 = Deno.core.encode("HTTP/1.1 101 Switching Protocols\r\n\r\n");
serve(async (req) => {
  const [conn, _firstPacket] = upgradeHttp(req);
  await conn.write(u8);
});
