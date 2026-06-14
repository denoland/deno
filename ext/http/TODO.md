# ACME auto-provisioning in `Deno.serve` -- remaining work

Status: working prototype. `Deno.serve({ acme: { domains, directoryUrl?,
contact?, cacheDir?, challengePort?, challengeHostname? } })` provisions a
certificate via RFC 8555 (`http-01` only), caches it on disk, renews at 2/3
of cert lifetime and swaps it without restarting the listener (via
`op_tls_cert_resolver_invalidate`). Tested end to end against the mock ACME
CA in `tests/util/server/servers/acme.rs` (spec test: `tests/specs/serve/acme`).

## Before this can land

- [ ] `03_acme.ts` accesses web APIs (`fetch`, `crypto`, `Deno.*`) via
      `globalThis` and has a file-level lint ignore for
      `prefer-primordials` / `no-explicit-any`. Convert to internal module
      references (`ext:deno_fetch/26_fetch.js`, `ext:deno_crypto/00_crypto.js`,
      fs ops) and primordials throughout.
- [ ] Unstable feature gating: the `acme` serve option should require an
      unstable flag (`--unstable-net`, or its own `--unstable-acme`).
- [ ] Decide the public API shape (currently `acme` on `ServeTcpOptions`):
      should this be WinterTC-able? Compare with Bun's approach.
- [ ] Credential/cache default location: `cacheDir` is currently opt-in
      (in-memory only without it). Consider defaulting to `$DENO_DIR/acme`
      so restarts don't burn CA rate limits by default.
- [ ] Renewal timer path has no end-to-end test (only the resolver
      invalidation op is covered). The mock CA could issue short-lived
      certs (or accept a validity hint) to exercise renewal in a spec test.
- [ ] In-flight resolution that completes after `invalidate()` can re-insert
      the old cert into the SNI cache. Benign today (the manager hands out
      the latest cert on re-resolve) but the cache insert should be
      generation-checked.
- [ ] Error UX: provisioning failures currently log via `internals.log` and
      handshakes fail with a generic resolver error. Surface the underlying
      ACME problem document to the user.
- [ ] ACME client hardening: respect `Retry-After` headers when polling,
      handle `processing` challenge status, external account binding (EAB)
      for CAs that require it (ZeroSSL), and rate-limit friendly backoff.
- [ ] Mock CA issues certs with fixed validity 2026-01-01..2028-01-01;
      needs bumping in 2027 (or derive from the current date).

## Blocked on TLS layer work

- [ ] TLS-ALPN-01 challenge support. Needs the cert resolver to see the
      client's ALPN offer per connection, to override the negotiated ALPN
      protocol for a single handshake (`acme-tls/1`), and to skip caching
      for challenge handshakes. See branch `feat/tls-clienthello-resolver`.
      With TLS-ALPN-01, provisioning works without binding a second port.
- [ ] DNS-01 challenge (required for wildcard domains). Needs a pluggable
      DNS provider hook in the `acme` options rather than TLS work.

## Related, not ACME-specific

- [ ] Public SNI-based cert resolution for `Deno.serve` / `Deno.listenTls`:
      the internal `unstableSniResolver` symbol callback should become a
      documented option (eg. `keyResolver(sni)`), independent of ACME.
