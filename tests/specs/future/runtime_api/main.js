console.log("window is", globalThis.window);
console.log(
  "Deno.FsFile.prototype.rid is",
  Deno.openSync(import.meta.filename).rid,
);

// TCP
// Since these tests may run in parallel, ensure this port is unique to this file
const tcpPort = 4509;
const tcpListener = Deno.listen({ port: tcpPort });
console.log("Deno.Listener.prototype.rid is", tcpListener.rid);
tcpListener.close();

// TLS
// Since these tests may run in parallel, ensure this port is unique to this file
const tlsPort = 4510;
const cert = Deno.readTextFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = Deno.readTextFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);
const tlsListener = Deno.listenTls({ port: tlsPort, cert, key });
console.log("Deno.TlsListener.prototype.rid is", tlsListener.rid);

try {
  new Deno.FsFile(0);
} catch (error) {
  if (
    error instanceof TypeError &&
    error.message ===
      "`Deno.FsFile` cannot be constructed, use `Deno.open()` or `Deno.openSync()` instead."
  ) {
    console.log("Deno.FsFile constructor is illegal");
  }
}

tlsListener.close();

self.close();
