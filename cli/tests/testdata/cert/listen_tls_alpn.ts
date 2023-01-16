const listener = Deno.listenTls({
  port: Number(Deno.args[0]),
  certFile: "./tls/localhost.crt",
  keyFile: "./tls/localhost.key",
  alpnProtocols: ["h2", "http/1.1", "foobar"],
});

console.log("READY");

const conn = await listener.accept() as Deno.TlsConn;
await conn.handshake();
conn.close();

listener.close();
