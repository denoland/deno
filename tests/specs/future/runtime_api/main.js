console.log("window is", globalThis.window);
console.log(
  "Deno.FsFile.prototype.rid is",
  Deno.openSync(import.meta.filename).rid,
);

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

self.close();
