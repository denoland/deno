import { assertRejects } from "@std/assert";

const listener = Deno.listenTls({
  port: Number(Deno.args[0]),
  cert: Deno.readTextFileSync("./tls/localhost.crt"),
  key: Deno.readTextFileSync("./tls/localhost.key"),
  alpnProtocols: ["h2", "http/1.1", "foobar"],
});

console.log("READY");

const conn = await listener.accept() as Deno.TlsConn;
await assertRejects(
  () => conn.handshake(),
  Deno.errors.InvalidData,
  "peer doesn't support any known protocol",
);
conn.close();

listener.close();
