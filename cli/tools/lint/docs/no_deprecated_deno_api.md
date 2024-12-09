Warns the usage of the deprecated - Deno APIs

The following APIs will be removed from the `Deno.*` namespace but have newer
APIs to migrate to. See the
[Deno 1.x to 2.x Migration Guide](https://docs.deno.com/runtime/manual/advanced/migrate_deprecations)
for migration instructions.

- `Deno.Buffer`
- `Deno.Closer`
- `Deno.close()`
- `Deno.Conn.rid`
- `Deno.copy()`
- `Deno.customInspect`
- `Deno.File`
- `Deno.fstatSync()`
- `Deno.fstat()`
- `Deno.FsWatcher.rid`
- `Deno.ftruncateSync()`
- `Deno.ftruncate()`
- `Deno.futimeSync()`
- `Deno.futime()`
- `Deno.isatty()`
- `Deno.Listener.rid`
- `Deno.ListenTlsOptions.certFile`
- `Deno.ListenTlsOptions.keyFile`
- `Deno.readAllSync()`
- `Deno.readAll()`
- `Deno.Reader`
- `Deno.ReaderSync`
- `Deno.readSync()`
- `Deno.read()`
- `Deno.run()`
- `Deno.seekSync()`
- `Deno.seek()`
- `Deno.serveHttp()`
- `Deno.Server`
- `Deno.shutdown`
- `Deno.stderr.rid`
- `Deno.stdin.rid`
- `Deno.stdout.rid`
- `Deno.TlsConn.rid`
- `Deno.UnixConn.rid`
- `Deno.writeAllSync()`
- `Deno.writeAll()`
- `Deno.Writer`
- `Deno.WriterSync`
- `Deno.writeSync()`
- `Deno.write()`
- `new Deno.FsFile()`

The following APIs will be removed from the `Deno.*` namespace without
replacement.

- `Deno.resources()`
- `Deno.metrics()`
