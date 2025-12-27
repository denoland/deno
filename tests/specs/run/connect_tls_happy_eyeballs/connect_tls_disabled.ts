// Test Deno.connectTls with autoSelectFamily disabled

const hostname = "localhost";

const listener = Deno.listenTls({
  hostname,
  port: 0,
  cert: Deno.readTextFileSync("./localhost.crt"),
  key: Deno.readTextFileSync("./localhost.key"),
});
const addr = listener.addr as Deno.NetAddr;

const serverPromise = (async () => {
  const conn = await listener.accept();
  const buf = new Uint8Array(5);
  await conn.read(buf);
  await conn.write(buf);
  conn.close();
})();

// Connect with autoSelectFamily disabled
const conn = await Deno.connectTls({
  hostname,
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

console.log("TLS connect with autoSelectFamily disabled: OK");
