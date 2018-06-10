- Fix v8_source_maps.ts so that we don't get random segfaults.

- Add os.statSync and os.tempDir- both are needed for the writeFileSync test in
  tests.ts

- Top-level await.

- Add ability to open TCP sockets and listen for connections.

- Add ability to receive HTTP connections (using net/http to parse)
  should try to use the same Request/Response types as fetch().

- Publish deno_testing to npm as a standalone module.

- Use mksnapshot instead of go-bindata.
