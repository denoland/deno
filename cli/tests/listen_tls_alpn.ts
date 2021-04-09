const listener = Deno.listenTls({
  port: 4503,
  certFile: "./cli/tests/tls/localhost.crt",
  keyFile: "./cli/tests/tls/localhost.key",
  alpnProtocols: ["h2", "http/1.1", "foobar"],
});

console.log("READY");

for await (const conn of listener) {
  conn.close();
}
