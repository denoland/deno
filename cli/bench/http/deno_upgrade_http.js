// TODO(bartlomieju): remove me, once `flash.upgradeHttp` is merged
// with `Deno.upgradeHttp`
const { upgradeHttp } = Deno.flash;
const u8 = Deno.core.encode("HTTP/1.1 101 Switching Protocols\r\n\r\n");
Deno.serve(async (req) => {
  const [conn, _firstPacket] = upgradeHttp(req);
  await conn.write(u8);
});
