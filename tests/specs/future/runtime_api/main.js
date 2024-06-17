console.log("window is", globalThis.window);
console.log("Deno.Buffer is", Deno.Buffer);
console.log("Deno.close is", Deno.close);
console.log("Deno.copy is", Deno.copy);
console.log("Deno.File is", Deno.File);
console.log("Deno.fstat is", Deno.fstat);
console.log("Deno.fstatSync is", Deno.fstatSync);
console.log("Deno.ftruncate is", Deno.ftruncate);
console.log("Deno.ftruncateSync is", Deno.ftruncateSync);
console.log("Deno.flock is", Deno.flock);
console.log("Deno.flockSync is", Deno.flockSync);
console.log(
  "Deno.FsFile.prototype.rid is",
  Deno.openSync(import.meta.filename).rid,
);
console.log("Deno.funlock is", Deno.funlock);
console.log("Deno.funlockSync is", Deno.funlockSync);
console.log("Deno.iter is", Deno.iter);
console.log("Deno.iterSync is", Deno.iterSync);
console.log("Deno.metrics is", Deno.metrics);
console.log("Deno.readAll is", Deno.readAll);
console.log("Deno.readAllSync is", Deno.readAllSync);
console.log("Deno.read is", Deno.read);
console.log("Deno.readSync is", Deno.readSync);
console.log("Deno.resources is", Deno.resources);
console.log("Deno.seek is", Deno.seek);
console.log("Deno.seekSync is", Deno.seekSync);
console.log("Deno.shutdown is", Deno.shutdown);
console.log("Deno.writeAll is", Deno.writeAll);
console.log("Deno.writeAllSync is", Deno.writeAllSync);
console.log("Deno.write is", Deno.write);
console.log("Deno.writeSync is", Deno.writeSync);

// TCP
// Since these tests may run in parallel, ensure this port is unique to this file
const tcpPort = 4509;
const tcpListener = Deno.listen({ port: tcpPort });
console.log("Deno.Listener.prototype.rid is", tcpListener.rid);

const tcpConn = await Deno.connect({ port: tcpPort });
console.log("Deno.Conn.prototype.rid is", tcpConn.rid);

tcpConn.close();
tcpListener.close();

// Unix
if (Deno.build.os === "windows") {
  console.log("Deno.UnixConn.prototype.rid is undefined");
} else {
  const socketPath = "./test.sock";
  const unixListener = Deno.listen({ transport: "unix", path: socketPath });

  const unixConn = await Deno.connect({ transport: "unix", path: socketPath });
  console.log("Deno.UnixConn.prototype.rid is", unixConn.rid);

  unixConn.close();
  unixListener.close();
  Deno.removeSync(socketPath);
}

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

const tlsConn = await Deno.connectTls({ port: tlsPort });
console.log("Deno.TlsConn.prototype.rid is", tlsConn.rid);

tlsConn.close();

const watcher = Deno.watchFs(".");
console.log("Deno.FsWatcher.prototype.rid is", watcher.rid);
watcher.close();

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

// Note: this could throw with a `Deno.errors.NotFound` error if `keyFile` and
// `certFile` were used.
const conn1 = await Deno.connectTls({
  port: tlsPort,
  certFile: "foo",
  keyFile: "foo",
});
conn1.close();
console.log("Deno.ConnectTlsOptions.(certFile|keyFile) do nothing");

// Note: this could throw with a `Deno.errors.InvalidData` error if `certChain`
// and `privateKey` were used.
const conn2 = await Deno.connectTls({
  port: tlsPort,
  certChain: "foo",
  privateKey: "foo",
});
conn2.close();
console.log("Deno.ConnectTlsOptions.(certChain|privateKey) do nothing");

tlsListener.close();

// Note: this could throw with a `Deno.errors.NotFound` error if `keyFile` and
// `certFile` were used.
try {
  Deno.listenTls({ port: tlsPort, keyFile: "foo", certFile: "foo" });
} catch (error) {
  if (
    error instanceof Deno.errors.InvalidData &&
    error.message ===
      "Deno.listenTls requires a key: Error creating TLS certificate"
  ) {
    console.log("Deno.ListenTlsOptions.(keyFile|certFile) do nothing");
  }
}
console.log("Deno.customInspect is", Deno.customInspect);

self.close();
