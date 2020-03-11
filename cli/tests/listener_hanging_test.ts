async function server(): Promise<void> {
  const l = Deno.listen({ port: 4444 });
  const buf = new Uint8Array(4);
  const conn = await l.accept();
  const process = async function(): Promise<void> {
    while (true) {
      const nr = await Promise.race([
        // Testing multiple read tasks!
        conn.read(buf),
        conn.read(buf),
        new Promise(resolve => {
          setTimeout(resolve, 100);
        })
      ]);
      if (!nr) {
        conn.close();
        return;
      } else {
        await conn.write(new Uint8Array([0, 1, 2, 3]));
      }
    }
  };
  await process();
  l.close();
}
server();

const conn = await Deno.connect({ port: 4444 });
async function reqRes(): Promise<void> {
  await conn.write(new Uint8Array([0, 1, 2, 3]));
  const buf = new Uint8Array(4);
  await conn.read(buf);
}
// First request-response:
await reqRes();
// Second request-response: expect error as conn is closed by server
setTimeout(async () => {
  try {
    await reqRes();
  } catch (e) {
    console.error(e);
  } finally {
    conn.close();
  }
}, 200);
