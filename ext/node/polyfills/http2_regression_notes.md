# node:http2 regression notes (grpc phase 3)

## Validation scope

Executed sequentially:

- `cargo build -p deno -p test_server`
- `cargo test --test specs grpc_js_phase1 -- --nocapture`
- `cargo test --test specs grpc_js_phase2_session -- --nocapture`
- `cargo test --test specs http2_session_semantics -- --nocapture`

Additional targeted spec:

- `cargo test --test specs http2_backpressure_and_ping -- --nocapture`
- `cargo test --test specs grpc_js_phase4_tls -- --nocapture`
- `cargo test --test specs grpc_js_phase4_mtls -- --nocapture`
- `cargo test --test specs tls_resolver_mtls -- --nocapture`
- `cargo test --test specs tls_request_cert_optional -- --nocapture`

## Pass/fail matrix

| Command | Result | Notes |
| --- | --- | --- |
| `cargo build -p deno -p test_server` | PASS | Build completed cleanly. |
| `cargo test --test specs grpc_js_phase1 -- --nocapture` | PASS | Sequential run. |
| `cargo test --test specs grpc_js_phase2_session -- --nocapture` | PASS | Sequential run. |
| `cargo test --test specs http2_session_semantics -- --nocapture` | PASS | Sequential run. |
| `cargo test --test specs http2_backpressure_and_ping -- --nocapture` | PASS | Added phase-3 stress coverage. |
| `cargo test --test specs grpc_js_phase4_tls -- --nocapture` | PASS | Secure grpc-js baseline. |
| `cargo test --test specs grpc_js_phase4_mtls -- --nocapture` | PASS | Strict no-cert rejection (`MTLS_NO_CERT_REJECT_OK`). |
| `cargo test --test specs tls_resolver_mtls -- --nocapture` | PASS | Resolver-backed TLS server path exercised. |
| `cargo test --test specs tls_request_cert_optional -- --nocapture` | PASS | `requestCert: true` + `rejectUnauthorized: false` behavior exercised (no-cert + untrusted-cert client attempts). |
| `cargo test -p unit_node_tests --test unit_node http2_test -- --nocapture` | PASS | Requires `tests/util/std` submodule initialized. |
| `./target/debug/deno test -A tests/unit_node/http2_test.ts` | FAIL (invocation mismatch) | Direct invocation resolves `@std/*` as package deps; use cargo unit harness command for repo coverage. |
| Parallel spec execution of fixed-port jobs | FAIL (expected) | `AddrInUse`; sequential rerun is required. |

## Known constraints / limitations

1. Spec jobs using fixed local ports cannot be run concurrently.
   - Concurrent runs can fail with `AddrInUse`.
   - Run sequentially for deterministic results.

2. `tests/util/std` must be initialized for repo unit harness coverage.
   - Required setup: `git submodule update --init --recursive tests/util/std`.
   - Without it, `cargo test -p unit_node_tests --test unit_node http2_test -- --nocapture` fails with missing `tests/util/std/*` imports.

3. Direct `deno test` execution of `tests/unit_node/http2_test.ts` does not match the repo harness wiring.
   - It resolves `@std/*` as package dependencies and fails unless separate package config is supplied.
   - Use the cargo unit harness command above for branch regression gating.

4. Native stream consumption is not default.
   - Default path uses socket-bridged transport.
   - Native consume requires `options.consumeNativeHttp2Stream === true`.
   - Broad non-grpc compatibility/performance coverage should be completed before considering a default flip.

5. TLS client-certificate enforcement for Node secure servers is backend-wired in this branch.
   - `node:tls` server `requestCert`/`rejectUnauthorized` options are forwarded to `Deno.listenTls` via internal symbols and enforced by the runtime TLS verifier path.
   - grpc-js mTLS gate requires a no-client-cert call to fail with `UNAVAILABLE` (`MTLS_NO_CERT_REJECT_OK`).

6. Resolver-backed TLS client-cert semantics are currently behavior-tested, not strict-gated.
   - `tls_resolver_mtls` verifies resolver path connectivity with client cert and captures current no-cert behavior.
   - The resolver path no-cert outcome is not yet pinned to strict rejection.

## Safety statement checkpoint

Explicit statement: **safe to keep socket-bridged as default for grpc-targeted insecure scenarios in this branch**, with mandatory sequential spec execution and the repo `unit_node http2_test` harness pass.
