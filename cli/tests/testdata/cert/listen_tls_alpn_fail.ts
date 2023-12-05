<<<<<<< HEAD
import { assertRejects } from "../../../../test_util/std/assert/mod.ts";
=======
import { assertRejects } from "../../../../test_util/std/testing/asserts.ts";
>>>>>>> 172e5f0a0 (1.38.5 (#21469))

const listener = Deno.listenTls({
  port: Number(Deno.args[0]),
  certFile: "./tls/localhost.crt",
  keyFile: "./tls/localhost.key",
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
