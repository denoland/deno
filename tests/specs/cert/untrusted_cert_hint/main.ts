// The test server's TLS cert is signed by a RootCA that isn't in Deno's
// default (Mozilla) trust store, so without `--cert` this fails with
// `UnknownIssuer` and should print the DENO_TLS_CA_STORE hint.
await fetch("https://localhost:5545/cert/cafile_ts_fetch.ts.out");
