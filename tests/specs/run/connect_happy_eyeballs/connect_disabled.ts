// Test Deno.connect with autoSelectFamily disabled

const listener = Deno.listen({ port: 0 });
const addr = listener.addr as Deno.NetAddr;

const serverPromise = (async () => {
  const conn = await listener.accept();
  const buf = new Uint8Array(5);
  await conn.read(buf);
  await conn.write(buf);
  conn.close();
})();

// Connect with autoSelectFamily explicitly disabled
const conn = await Deno.connect({
  hostname: "127.0.0.1",
  port: addr.port,
  autoSelectFamily: false,
});

await conn.write(new TextEncoder().encode("hello"));
const buf = new Uint8Array(5);
await conn.read(buf);
console.log("Received:", new TextDecoder().decode(buf));

conn.close();
listener.close();
await serverPromise;

console.log("Connect with autoSelectFamily disabled: OK");
