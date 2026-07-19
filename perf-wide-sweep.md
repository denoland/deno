# Wide-sweep perf opportunities (cheap + big, high confidence)

Codebase-wide sweep (2026-07-18) complementing the two existing docs:
`optimize-runtime.md` (deno_core tick/boundary) and
`allocator-opportunities2.md` (Rust allocator/hasher wins). Nothing below
overlaps those. Line numbers are against the current `feat/musl-builds`
checkout. Analysis only; nothing implemented.

Every Tier 1 item was verified by reading the code directly; the "confirmed"
notes say exactly what was checked.

**Status 2026-07-19** — PRs opened: #1 → 36159, #3 → 36160 (plus a fix for
the missing-await race in `_fs_rmdir_test` that the new open timing
exposed), #5+#10 → 36164, #8 → 36161 (tcp/vsock/unix; tunnel skipped, its
listener is an external crate), #9 → 36165, #11 → 36166, #4 → 36167 (with
`__node_internal_` elision added to core `format_stack_trace`). #2 was
retracted (std already pre-sizes). Remaining: #6 (node-shim PATH scan;
needs a lazy-vs-cached decision) and #7 (stream drain sync fast path).

---

## Tier 1 — verified, contained, do these

### 1. `readFile` copies the whole file an extra time

**Where:** `ext/fs/ops.rs:1561` (sync) and `:1609` (async):
`Ok(buf.into_owned().to_vec().into())`.

`buf` is a `Cow<'static, [u8]>`; `into_owned()` already yields the owned
`Vec<u8>`. The subsequent `.to_vec()` is a guaranteed second full-file
allocation + memcpy before the zero-copy `Uint8Array: From<Vec<u8>>` conversion.
There is already a `todo(#27107): do not clone here` on both lines.

**Fix:** `Ok(buf.into_owned().into())`. `From<Vec<u8>>` goes through
`into_boxed_slice()`, which is free here because std pre-sizes the read buffer
exactly (see retracted item #2 below). DONE: PR opened from branch
`perf/readfile-extra-copy`.

**Impact:** every `Deno.readFile()`/`readFileSync()` and node `fs.readFile`; a
50 MB read currently does a 50 MB extra memcpy + transient allocation.

### 2. ~~`read_file` / `read_all` don't pre-size the buffer~~ RETRACTED

Invalid. Rust std (since the `File::read_to_end` specialization; confirmed in
the 1.95 toolchain, `library/std/src/fs.rs` `buffer_capacity_required`) already
pre-sizes the buffer from fstat + stream position when `read_to_end` is called
directly on a `File`, and probes EOF with a stack buffer so capacity stays
exact. `Vec::new()` + `file.read_to_end(&mut buf)` in `std_fs.rs:483,494` and
`ext/io/lib.rs:972,987` is therefore already optimal.

### 3. Async `readFile`/`writeFile` run the blocking `open()` on the event-loop thread

**Where:** `ext/fs/std_fs.rs:492` (`read_file_async`) and `:464`
(`write_file_async`) — `open_with_checked_path(...)` executes _before_
`spawn_blocking`; only the read/write body is offloaded. Contrast `open_async`
(`:117`), which correctly opens inside `spawn_blocking`.

**Fix:** move the open into the closure (move `CheckedPathBuf` in).

**Impact:** N concurrent `readFile`s currently serialize their `open(2)`s on the
runtime thread; a slow open (network FS) stalls the whole event loop. Confirmed:
error propagation is unchanged since errors flow through the spawned task either
way.

### 4. `hideStackFrames` wraps every hot validator in a closure

**Where:** `ext/node/polyfills/internal/hide_stack_frames.ts:23-33`. Applied to
~55 functions including `validateFunction`, `validateInt32`, `validateBuffer`,
`getValidatedPath`, `getValidatedFd`, `validateOffsetLengthRead/Write` — i.e.
the innermost glue of nearly every `node:fs`/`net`/`http`/stream call. Each call
through a wrapper materializes a rest-args array + try/catch + `ReflectApply`,
and they nest (`getValidatedPath` → `validatePath` → `nullCheck` = 3 wrappers;
one `fs.read()` crosses 6+).

**Fix:** modern Node dropped the wrapper — `hideStackFrames` now only renames to
`__node_internal_<name>` and relies on frame elision by name. Deno already does
the rename (line 34-35) and `runtime/fmt_errors.rs:319` already elides
`__node_internal_` frames. Drop the wrapper, `return fn`.

**Caveat (the one real risk):** the wrapper's `ErrorCaptureStackTrace` also
cleans stacks read _programmatically_ via `err.stack`; Rust-side elision only
covers Deno's error display. Check node-compat tests asserting stack shapes; if
needed, also filter `__node_internal_` in the JS `prepareStackTrace` path the
way Node does.

**Impact:** removes several allocations + call frames from every fs/stream/
http/nextTick call in node-compat code; benefits express-style servers and
fs-heavy tooling.

### 5. `process.nextTick` allocates an args array even with no extra args

**Where:** `ext/node/polyfills/_next_tick.ts:48-84`. Rest param `...args`
materializes an array on every call, then re-copies into `args_`. The common
`nextTick(cb)` case should allocate nothing — Node deliberately uses `arguments`
for exactly this reason. There is already a
`TODO(bartlomieju): seems superfluous` at line 65.

**Fix:** `function nextTick(callback)` + switch on `arguments.length`, building
`args_` from `arguments` only when extra args exist.

**Impact:** `nextTick` fires per stream read/write completion and throughout the
node http lifecycle; this is pure GC-pressure removal on every node-compat
workload.

### 6. Node-shim setup scans PATH + canonicalizes on every startup

**Where:** `cli/node_compat_shim.rs:178-215` (`ensure_node_on_path`), called
from `cli/lib.rs:982-1000` on every `run`/`task`/`test`/`bench`/`eval`/
`repl`/`serve` invocation.

Confirmed: the only early-outs are the env-disable and the re-entry env var. A
normal shell invocation always pays `which::which("node")` — a stat per PATH
entry. When no real node is installed (the case the shim exists for), it
additionally pays `current_exe()` + `canonicalize` + `read_link` + possibly a
second `canonicalize` — _even when the shim already exists and is valid_
(`shim_is_valid` at `:204` only gates creation, not the scan).

**Fix:** make shim setup lazy — do it when a child process actually attempts to
spawn `node` (most scripts never do). Failing that, cache the "shim valid + PATH
prepended" decision to skip the scan/canonicalize on the common path.

**Impact:** ~10-20 stats + readlink/realpath removed from every cold and warm
`deno run` startup; worst on long PATHs and network/virtualized filesystems.

### 7. Stream→resource drain pays 2 promise allocs + a microtask per chunk

**Where:** `ext/web/06_streams.js:968` (`readableStreamWriteChunkFn` is `async`)
and its call site `:1035-1039` (`PromisePrototypeThen(...)`).

Confirmed: the common case — `op_readable_stream_resource_write_sync` returns 1
(sync write complete) — still allocates the async function's promise, plus the
`.then` reaction, plus a forced microtask hop, per chunk. This is the drain loop
under every streamed `Deno.serve` response body and streamed fetch upload. The
surrounding read machinery was already hoisted per-stream (see comment at
`:1013-1017`); the write fn is the remaining per-chunk cost.

**Fix:** make it a plain function returning `true`/`false` synchronously for the
res 1/0 cases and a promise only for res 2 (backpressure) and cancel; loop
synchronously unless the return is a thenable.

**Impact:** streaming response/upload throughput; pairs well with
`optimize-runtime.md` #3 (they attack the same per-chunk promise churn from both
sides).

### 8. TCP accept does a redundant `getpeername` per connection

**Where:** `ext/net/ops.rs:223` — `accept()` already returns the peer address,
which is bound as `_socket_addr` and discarded; `:237` then calls
`tcp_stream.peer_addr()`, an extra syscall per accepted connection.

**Fix:** use the address `accept()` returned. One-liner.

### 9. Per-chunk allocations in TextDecoderStream / TextEncoderStream

**Where:** `ext/web/08_text_encoding.js:378-381` — the transform allocates
`{ allowShared: true }` and `{ stream: true }` object literals per chunk; the
module already hoists the identical pattern at `:345` (`encodeIntoOpts`). And
`:477` — `TextEncoderStream` runs `webidl.converters.DOMString(chunk)` per chunk
with no `typeof chunk !== "string"` guard, unlike `TextEncoder.encode` (`:280`).

**Fix:** hoist both option objects to module constants; add the string typeof
guard. Trivially safe.

**Impact:** every piped text decode (response bodies, SSE, stdin) and encode.

### 10. Per-chunk `constructor.name === "ReusedHandle"` string compare on sockets

**Where:** `ext/node/polyfills/internal/stream_base_commons.ts:120` (per write
completion) and `:275` (per read chunk); also `net.ts:511` (per connection,
minor).

**Fix:** brand `ReusedHandle` with a symbol (or compare
`stream.constructor === ReusedHandle` by reference) instead of materializing and
comparing the `.name` string per chunk.

**Impact:** node:net/http servers, per-chunk; small constant but on the hottest
socket path.

### 11. `fetch` re-parses its URL per hop just to read scheme + port

**Where:** `ext/fetch/26_fetch.js:397` — `new URL(req.currentUrl())` allocates a
URL and crosses to the Rust parser once per fetch call and per redirect hop,
solely for the bad-port/scheme check on an already-validated URL string.

**Fix:** lightweight string scan for scheme/port (pattern exists in
`ext/web/00_url.js`), or cache parsed components on the inner request. Small
absolute win (fetch is network-bound) but purely redundant work.

---

## Tier 2 — real, but need a design decision or more verification

- **`loaded_files` URL clone + insert per module load**
  (`cli/module_loader.rs:1354`, set at `:596`): only consumer is the
  dynamic-import reload check (`:1049`); programs without dynamic imports pay a
  Url clone + hash insert per module for nothing. Fix: populate lazily when the
  graph has dynamic-import edges. (Agent-verified call sites; not re-read by
  me.)
- **`FsFile` async ops `try_clone()` (dup+close) per op** (`ext/io/lib.rs:678`):
  2 syscalls per async read/write/seek/stat exist only so _sync_ ops can
  interleave with an in-flight async op. Removing it changes interleaving
  semantics to `FileBusy` — needs a deliberate call.
- **HTTP parser wraps the body buffer per execute**
  (`ext/node/polyfills/_http_server.js:726-732`): `Buffer.from` wrap per parser
  execute serves only the rare CONNECT/Upgrade branch (`:807`); defer the wrap
  into that branch. Verify no other consumer of `_lastRawPacket` relies on
  Buffer-ness (error path uses it).
- **Fresh `TextDecoder` per Buffer-path fs call**
  (`ext/node/polyfills/internal/fs/utils.mjs:912-922`, also `_fs/_fs_dir.ts:42`,
  `fs.ts:889`): hoist one module-level decoder. Only hits Buffer/Uint8Array
  paths (string paths, the common case, unaffected).
- **Event dispatch allocates DOM-tree machinery Deno never uses**
  (`ext/web/02_event.js:563-728`): `getParent()` is always null, path length is
  always 1, yet every dispatch builds `touchTargets = []`, a 7-field path tuple,
  and runs both phase loops. A null-parent fast path helps WebSocket-message and
  worker-messaging throughput, but must preserve `composedPath()` /
  `stopImmediatePropagation` semantics — medium risk.
- **V8 code-cache key re-hashes full emitted source per module per run**
  (`cli/module_loader.rs:790-808`): could reuse the emit cache's hash, but the
  keys hash different bytes (original vs emitted) — needs plumbing, not a
  drop-in.
- **`DENO_DIR` resolved + canonicalized twice per run** (`cli/lib.rs:983-989` vs
  `libs/resolver/factory.rs:~360`): thread one resolution through.
- **`node_analysis_db` sqlite warmed for pure-ESM programs**
  (`cli/factory.rs:416`): off-thread already, so mostly overlapped; gate on
  node/CJS actually being in play if touching that code anyway.

---

## Verified non-issues (checked, deliberately not flagged)

- TCP/TLS read/write ops: single syscall per op; TLS post-write `flush()` is
  required by tokio-rustls. Stat ops already use the `&mut [u32]` buffer
  protocol.
- node `Buffer`, socket Rust path (`stream_wrap.rs`): slab pooling,
  `uv_try_write` fast path, zero-copy iovecs already in place.
- `EventEmitter.emit` rest-args, chunked-encoding write pattern: faithful to
  Node's own design, not Deno regressions.
- Headers/Body/serve JS fast paths, streams read-request hoisting,
  `op_read_all`: already tuned.
- Warm-path module loading: no double disk reads; graph source text reused via
  Arc; cache DBs opened off-thread; `maybe_npm_install` gated.

## Suggested batching

1. **fs batch:** #1 + #2 + #3 (one PR, `ext/fs` + `ext/io`; easy to benchmark
   with a readFile loop).
2. **node-glue batch:** #4 + #5 + #10 (one PR; run node-compat suite for
   stack-shape assertions).
3. **one-liners:** #8, #9, #11 (individually trivial PRs).
4. **streams:** #7 (own PR; needs backpressure-path care + streams WPT).
5. **startup:** #6 (own PR; decide lazy-vs-cached shim strategy).
