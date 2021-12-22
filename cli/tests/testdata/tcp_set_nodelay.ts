const client = await Deno.connect({
  hostname: "127.0.0.1",
  port: 4245,
  transport: "tcp",
});
client.setNoDelay(true);
await client.write(new Uint8Array([1, 2, 3]));
client.closeWrite();
const buf = new Uint8Array(1024);
await client.read(buf);
await Deno.stdout.write(buf);
