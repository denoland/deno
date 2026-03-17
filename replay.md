# `deno --record` / `deno replay` Design Document

## Overview

Deterministic record-and-replay as a runtime mode. Recording is a transparent flag on `deno run`. Replay is a dedicated subcommand that reconstructs past executions and integrates with Chrome DevTools for time-travel debugging.

No APIs. No imports. No code changes.

---

## Recording

### Usage

```bash
deno run --record=trace.bin server.ts
deno run --record=trace.bin --record-limit=500mb server.ts
```

### What Gets Recorded

Every source of nondeterminism that flows through Deno's ops layer:

| Category | Ops | Recorded Data |
|----------|-----|---------------|
| Time | `Date.now()`, `performance.now()` | Returned timestamp |
| Randomness | `Math.random()`, `crypto.getRandomValues()` | Returned bytes |
| Network | `fetch`, `Deno.connect`, `Deno.listen` | Request/response pairs, byte streams |
| Filesystem | `Deno.readFile`, `Deno.stat`, `Deno.readDir` | Return values, file contents |
| Subprocess | `Deno.Command` | stdout, stderr, exit code |
| Environment | `Deno.env.get`, `Deno.hostname` | Returned strings |
| Timers | `setTimeout`, `setInterval` | Scheduling order and fire sequence |
| Async scheduling | Event loop | Op completion order |

### What Does NOT Get Recorded

- CPU-bound computation (deterministic given same inputs)
- V8 internals (JIT, GC, IC — these are invisible to JS)
- Module source code (captured by reference via content hash)

### Trace Format

Binary format, streaming. The runtime appends entries as ops complete — no buffering entire responses in memory.

```
┌─────────────────────────────────────────┐
│ Header                                  │
│  magic: "DENO_TRACE\0"                 │
│  version: u32                           │
│  deno_version: string                   │
│  v8_version: string                     │
│  arch: string                           │
│  os: string                             │
│  module_graph_hash: [u8; 32]            │
│  permissions: PermissionSet             │
│  recorded_at: u64 (unix ms)             │
│  entry_point: string                    │
├─────────────────────────────────────────┤
│ Module Table                            │
│  [specifier, content_hash, size]...     │
├─────────────────────────────────────────┤
│ Op Events (streaming, append-only)      │
│  ┌───────────────────────────────┐      │
│  │ sequence_id: u64              │      │
│  │ timestamp_us: u64             │      │
│  │ op_name: interned string      │      │
│  │ async_id: u64                 │      │
│  │ payload_len: u32              │      │
│  │ payload: [u8]  (V8 serialized)│      │
│  └───────────────────────────────┘      │
│  ...repeated...                         │
├─────────────────────────────────────────┤
│ Checkpoints (periodic, for fast seek)   │
│  [sequence_id, file_offset]...          │
└─────────────────────────────────────────┘
```

### Checkpoints

Every N ops (configurable, default ~10,000), the runtime writes a checkpoint: a V8 heap snapshot at that point plus the file offset. This enables fast seeking during replay — instead of replaying from the start, jump to the nearest checkpoint and replay forward.

### Ring Buffer Mode

```bash
deno run --record=trace.bin --record-limit=500mb server.ts
```

Keeps only the most recent N megabytes of trace data. When the limit is hit, old op events are discarded (but the header and module table are preserved). Useful for long-running servers where you only care about the last few minutes before a crash.

### Recording Overhead

Target: < 5% CPU overhead, bounded memory.

This is achievable because:
- Ops already return structured data — we're copying it, not intercepting syscalls
- V8 serialization is fast for the small payloads most ops return
- File I/O for the trace is append-only and can be buffered
- No binary translation, no JIT interposition, no ptrace

---

## Replay

### Usage

```bash
# Basic replay — re-executes the recorded program deterministically
deno replay trace.bin

# Replay with DevTools debugging
deno replay trace.bin --inspect
deno replay trace.bin --inspect-brk  # pause at first statement

# Print a summary without replaying
deno replay trace.bin --info

# Validate trace against current source (are modules unchanged?)
deno replay trace.bin --validate
```

### How Replay Works

1. **Load header** — verify deno/v8 version compatibility, check module hashes
2. **Restore module graph** — load modules from disk (hashes must match, or error)
3. **Execute with ops intercepted** — instead of performing real I/O, every op returns the next recorded payload from the trace
4. **Enforce scheduling order** — async ops complete in the exact sequence they were recorded, regardless of actual timing

The replayed program observes the exact same world as the original execution. `Date.now()` returns the recorded timestamp. `fetch()` returns the recorded response. `Math.random()` returns the recorded value. From JS's perspective, time has not passed and the network has not changed.

### Divergence Detection

If the replayed code takes a different path than the recording (e.g. modules changed, or a bug in the replay engine), the runtime detects the divergence:

```
error: Replay divergence at op #48,291
  Expected: op_fetch_send (async_id: 1042)
  Got:      op_read_file (async_id: 1043)

  This usually means source code has changed since the recording.
  Run `deno replay trace.bin --validate` to check module hashes.
```

---

## DevTools Integration

### Connecting

```bash
deno replay trace.bin --inspect
# Debugger listening on ws://127.0.0.1:9229/...
# Open chrome://inspect in Chrome
```

The replay session speaks the standard Chrome DevTools Protocol (CDP). Any CDP client works — Chrome, VS Code, WebStorm.

### Standard Debugging (works immediately)

Everything you'd expect from `--inspect` works during replay:
- Breakpoints (line, conditional, logpoint)
- Step over / into / out
- Scope inspection, watch expressions
- Console evaluation
- Call stack navigation
- Source maps

The difference: breakpoints are perfectly reproducible. Hit "restart" and you land in the exact same state with the exact same data.

### Time-Travel Controls (new CDP extensions)

Replay adds new capabilities to the debugger:

#### Reverse Continue

Run backwards from the current pause point to the previous breakpoint hit. Implemented by: find the nearest checkpoint before the target, replay forward to that point.

```
CDP: Debugger.reverseContinue
```

#### Step Back

Step to the previous statement. Same mechanism — checkpoint + forward replay — but the UX is a single "step back" button.

```
CDP: Debugger.stepBack
```

#### Seek to Op

Jump to any recorded op by sequence ID. The DevTools timeline shows all ops; clicking one seeks to that point in execution.

```
CDP: Runtime.seekToOp { sequenceId: 48291 }
```

#### Async Causal Chain

Given any op, walk backwards through the chain of async operations that caused it. "This fetch was awaited by this function, which was called from this event handler, which was triggered by this timer, which was set during module init."

```
CDP: Runtime.getAsyncCausalChain { asyncId: 1042 }
→ [
    { asyncId: 1042, op: "op_fetch_send", location: "server.ts:42" },
    { asyncId: 891,  op: "op_timer",      location: "server.ts:38" },
    { asyncId: 0,    op: "top_level",     location: "server.ts:1" }
  ]
```

### DevTools UI Extensions

Chrome DevTools supports custom panels via extensions. Deno could ship a companion extension that adds:

**Timeline Panel** — a horizontal timeline of all recorded ops, color-coded by category (net=blue, fs=green, timer=yellow). Click any op to seek. Zoom in/out. Shows wall-clock time from the recording.

**Async Graph** — a visual DAG of async causality. Nodes are ops, edges are "caused by" relationships. Click a node to seek and inspect.

**Op Inspector** — when paused at a breakpoint, shows the recorded ops that are "in flight" at that moment, with their full request/response payloads.

---

## CLI Reference

### `deno run --record`

```
FLAGS:
  --record=<path>
      Enable recording. Writes trace to the specified file.
      File is created or overwritten.

  --record-limit=<size>
      Maximum trace size. Ring-buffer mode — keeps the most
      recent data within the budget. Accepts kb, mb, gb suffixes.
      Default: unlimited.

  --record-filter=<categories>
      Comma-separated list of op categories to record.
      Categories: net, fs, time, random, env, subprocess, timer, all.
      Default: all.

  --record-checkpoint-interval=<n>
      Write a heap checkpoint every N ops.
      Lower = faster seeking, larger trace.
      Default: 10000.
```

### `deno replay`

```
USAGE:
  deno replay <trace-file> [FLAGS]

FLAGS:
  --inspect
      Open a CDP debugging session. Connect Chrome DevTools
      or any CDP client.

  --inspect-brk
      Same as --inspect, but pause before the first statement.

  --inspect-addr=<host:port>
      Address for the CDP WebSocket. Default: 127.0.0.1:9229.

  --info
      Print trace metadata and exit. Shows: entry point,
      deno/v8 version, duration, op count, module list,
      permissions, file size.

  --validate
      Check module hashes against source on disk. Reports which
      files have changed since the recording. Does not replay.

  --seek=<sequence_id>
      Start replay paused at the given op sequence ID.
      Useful for jumping directly to a known point of interest.

  --speed=<multiplier>
      Replay wall-clock speed relative to recorded time.
      Default: max (as fast as possible).
      Use --speed=1 for real-time, --speed=0.5 for half-speed.
```

---

## Interaction with Other Features

### Permissions

Replay does not perform real I/O, so no permissions are needed. The `deno replay` subcommand ignores `--allow-*` flags — it's replaying recorded data, not accessing the system.

### Snapshots (`Deno.snapshot`)

Recording and snapshots are complementary:
- A snapshot captures a *state* for fast restore
- A recording captures an *execution* for debugging

A snapshot could be embedded as the first checkpoint in a trace, giving instant seek to "just after initialization."

### `--watch` Mode

`--record` and `--watch` are mutually exclusive. Restarting on file change would create a new execution — start a new recording instead.

### Testing

```bash
deno test --record=test-trace.bin
```

Records the entire test run. On failure, replay the trace to debug the exact conditions that caused the failure — including any randomness, timing, or network responses.

---

## Open Questions

1. **Trace portability** — can a trace recorded on Linux be replayed on macOS? The ops return the same types, but path separators, env vars, etc. differ. Probably: yes for most ops, no for fs paths.

2. **Worker threads** — workers are separate isolates. Record each worker's ops independently, with a shared sequence counter for cross-worker ordering?

3. **FFI** — `Deno.dlopen` calls bypass the ops layer entirely. Options: refuse to record programs using FFI, or record at the FFI boundary (return values only, not internal native state).

4. **Trace sharing** — recordings contain full network responses, file contents, env vars. Sensitive data. Need a `deno replay trace.bin --redact` that strips payloads but preserves structure for sharing bug reports?

5. **Partial replay** — can you replay just a single request to a server, rather than the entire execution from startup? Requires identifying the async tree rooted at that request and replaying only those ops.
