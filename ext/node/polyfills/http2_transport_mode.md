# node:http2 transport mode (`consumeNativeHttp2Stream`)

## Summary

`node:http2` now defaults to socket-bridged transport in the JS layer:

- outbound frame bytes are emitted from the runtime-backed HTTP/2 session via `handle.onwrite`
- bytes are written through the bound `net.Socket` write path
- inbound bytes are fed back to the runtime-backed session via `handle.receive(...)`

Native handle consumption remains available only when explicitly enabled:

- `options.consumeNativeHttp2Stream === true`

## Rationale

The default was changed to avoid lifecycle mismatches seen with detached native socket handles:

- detached close/shutdown paths diverged from Node behavior in compatibility tests
- callback ordering was more fragile under grpc keepalive/ping/close races
- fallback transport allows backpressure handling through socket `"drain"` and keeps socket/session observability aligned with Node-facing APIs

## Semantics and constraints

- This switch does **not** replace runtime-backed HTTP/2 logic. Session state, settings, ping, flow-control, and stream/frame handling still come from the upstream backend (`ext/node/ops/http2/*`).
- `consumeNativeHttp2Stream` is intended as an explicit compatibility/performance experiment flag, not a default path for general users at this stage.
- Re-entrant backend ops are required for JS callbacks that schedule async work during frame send/receive processing.

## When to revisit defaulting native consume

Before re-enabling by default, validate:

1. socket close/shutdown parity with Node under grpc and non-grpc `http2` traffic
2. callback ordering stability across ping/cancel/close races
3. no regressions in broad `node:http2` coverage beyond grpc-targeted specs
