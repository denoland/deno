# P3 — installation shape and cost, under a snapshot

Throwaway prototype for [wayfinder P3](https://github.com/bartlomieju/wayfinder/issues/28),
grown out of the [P1](https://github.com/bartlomieju/wayfinder/issues/19) cut.
Delete this directory and the branch once the findings are folded into the spec.

**Question.** Can Channel 1 be installed at all in a snapshotted runtime, and at
what steady-state cost? P1 measured a *masking* named property handler in a
*debug* build at ~164 ns/op on every named global read, and could only run under
`--features hmr` because `create_context` skips `global_template_middleware`
when booting from a snapshot.

**Verdict.** Yes to installation, three ways, and P1's cost verdict does not
survive. The tax P1 measured is real and structural — but it is a property of
the *masking* handler P1 happened to build, not of the mechanism. Two shapes
avoid it almost entirely, and one of them is also the only shape that can be
switched off at runtime.

## The prior art P1 and D3 both missed

`ext/node/global.rs`, removed in
[denoland/deno#33249](https://github.com/denoland/deno/pull/33249) (2026-04-29).
The same mechanism, shipped in production Deno for ~3 years, boot-from-snapshot
included. It was removed because it had become a **no-op** — #33118 made Node
timers the default, leaving only `process` (already on `globalThis`) and
`window` (never populated) — *not* because of cost. There is no "we tried it and
it was too slow" verdict to inherit.

Three things it establishes for free:

- **`current_mode()` read `Isolate::GetCurrentHostDefinedOptions()` at index 0**
  and branched on it, on every guarded global access. D3's Channel 1 attribution
  read is not novel and not unproven; it shipped.
- **`PropertyHandlerFlags::NON_MASKING | HAS_NO_SIDE_EFFECT`**, with a source
  comment claiming this is what makes it cheap. P1's cut set no flags at all.
- **Key matching without allocation**: UTF-16 length pre-filter against const
  min/max, `write_v2` into a stack buffer, binary search over a sorted const
  array. P1's `to_rust_string_lossy` per access is the naive shape it already
  disclaimed.

The external-references cost P3 anticipated is the concrete seven-entry
`ExternalReference { named_getter: … }` block in `ext/node/lib.rs`'s
`customizer`.

## How to run

```
cd prototype-p1
./run_p3.sh build     # three release binaries, ~8 min each
./run_p3.sh bench     # the cost matrix
./run_p3.sh startup   # hyperfine, hello-world and the express tree
```

Variants:

| variant | how it is installed | build-time? |
|---|---|---|
| `off` | nothing | — |
| `mask` | masking named property handler on the global object template (P1's shape) | **yes**, `--features permcap_mask` |
| `nonmask` | NON_MASKING handler + guarded names moved to a side bag (ext/node's shape) | **yes**, `--features permcap_nonmask` |
| `accessor` | per-property getters on the live global via `define_property` | **no**, `PERMCAP_ACCESSOR=1` |

## Findings

### 1. Cost — the masking handler is the problem, and only the masking handler

Release build, aarch64 macOS, best of 3 runs × 5M iterations, ns/op. `nonmask
(bare)` is the handler present with nothing removed from the global; `nonmask
(bag)` additionally moves guarded names into the side bag.

| read | off | nonmask (bare) | nonmask (bag) | accessor | mask no-op | mask fast | mask naive |
|---|---|---|---|---|---|---|---|
| `globalThis.fetch` (guarded) | 0.50 | 6.66 | 256.49 | 281.22 | 81.37 | 307.85 | 350.24 |
| `globalThis.Math` (**unguarded**) | 0.60 | 6.66 | 6.67 | **0.50** | 81.57 | 100.83 | 149.04 |
| bare `Math` (free name, unguarded) | 0.50 | **0.50** | **0.51** | **0.51** | 41.45 | 50.42 | 75.05 |
| bare `fetch` (free name, guarded) | 0.51 | 0.51 | 256.11 | 288.43 | 41.48 | 236.71 | 259.92 |
| absent property | 4.37 | 11.54 | 23.73 | 4.38 | 99.68 | 122.74 | 168.25 |
| local variable baseline | 0.51 | 0.51 | 0.50 | 0.50 | 0.66 | 0.66 | 0.66 |

**The masking handler's tax survives release optimization and is structural.**
A *no-op* masking callback still costs 41.45 ns on a bare `Math` read (83×
baseline) and 81.57 ns on `globalThis.Math` (136×). P1 called this correctly:
V8 abandons its global-load inline caches, and no amount of callback tuning
touches it. P1's own caveat — "a release build with cached `v8::Global<v8::String>`
comparisons would shrink both columns substantially" — is half right: the *fast*
key match is worth ~30% (mask fast 50.42 vs mask naive 75.05 on bare `Math`),
and the IC loss is untouched, exactly as P1 predicted.

**NON_MASKING removes essentially all of it.** Free-name reads are *unchanged*
at 0.50 ns — the global-load IC survives intact — and explicit `globalThis.X`
property reads cost 6.66 ns. Deno's own claim on `ext/node/global.rs` is
measured true. The distance between P1's verdict ("cost is the mechanism's real
problem") and a viable mechanism is one flag P1's cut did not set.

**Accessors are better still.** Unguarded reads are at baseline in both forms
(0.50 / 0.51 ns) because there is no handler on the global object at all — only
the guarded names carry a getter, so nothing unrelated is even consulted.

**What a guarded read costs is the callback, not the property shape.** 256.49
(nonmask+bag) vs 281.22 (accessor) for the same work: cross into Rust, read
host-defined options, look up the bag. Both shapes pay the same order. If that
~270 ns ever matters it is a callback-optimization problem, and it is bounded by
real usage: P1 traced the whole express graph and found **5 packages of the
resolved tree ever read a guarded global**, 1117 reads total during startup.
At ~270 ns that is ~0.3 ms. The residual risk is a hot loop over an authority
global, not the ambient cost.

### 2. Installation under a snapshot — yes, three different ways

**The interceptor reaches a snapshotted context by being built into it.**
`create_context` (`jsruntime.rs:2813`) only runs `global_template_middleware` on
the no-snapshot path, so the handler has to be installed when the snapshot is
*created* — i.e. the extension must be in `runtime/snapshot.rs`'s list, and its
callback must be in the external-references table. This works, and is how
ext/node shipped it.

**`global_object_middleware` runs unconditionally** — `jsruntime.rs:2838` sits
outside the has-snapshot branch — so anything installed there survives
deserialization. But it is too early to be useful: at that point the
authority-bearing globals have not been defined yet (bootstrap JS defines them),
so there is nothing to wrap. The working install point is **post-bootstrap**,
where plain `define_property` on the live global is enough.

### 3. Installation is a build-time property for the interceptor, and a runtime one for accessors — this contradicts D4

Because the handler is baked into the snapshot, **its presence cannot be gated
by a config file, an env var, or a flag.** `permcap_mask` and `permcap_nonmask`
had to be cargo features for exactly this reason. Whatever the interceptor
costs, every Deno user pays it, confined app or not.

That is in direct tension with [D4](https://github.com/bartlomieju/wayfinder/issues/20),
which made the config table's own presence the gate — "no table → feature off".
For an interceptor shape, "off" is not expressible. For accessors it is: they go
on the live global at runtime, need no build-time feature and no external
reference, and the same binary runs both ways. **This is the strongest argument
for accessors, and it is an argument D3 did not have.**

### 4. Startup cost is unmeasurable — and the first measurement of it was wrong

hyperfine, mean ± σ, uniform `env` prefix on every row:

| | hello-world | express + lodash + chalk |
|---|---|---|
| off | 26.9 ± 2.2 ms | 98.8 ± 1.9 ms |
| accessor | 26.4 ± 2.5 ms | 100.9 ± 5.9 ms |
| mask (handler present, callback off) | 25.6 ± 1.9 ms | 99.0 ± 2.4 ms |
| mask (fast) | 25.7 ± 1.2 ms | 101.3 ± 5.8 ms |
| nonmask (handler present, callback off) | 26.2 ± 2.7 ms | 100.4 ± 5.7 ms |
| nonmask (fast) | 25.6 ± 1.8 ms | 98.7 ± 2.4 ms |

Every variant is within ±3% and inside σ. Note this does **not** contradict
finding 1: bootstrap does not run a five-million-iteration global-read loop.

**Recorded because it nearly became a finding:** the first pass showed every
`PERMCAP=1` row costing ~+8.5 ms, a near-doubling of hello-world startup. That
was `/usr/bin/env`, which only the `PERMCAP` rows carried. On macOS the extra
exec costs more than the entire mechanism. Controlled for it (`env
PERMCAP_UNUSED=1` on the baseline) the delta vanished to zero.

### 5. D2's OpState current-package cell is close to free

`op_permcap_probe(bracket)` — a cheap sync op, with and without D2's
set/restore plus a grant-table resolution:

| binary | bare op | + D2 bracket | delta |
|---|---|---|---|
| off | 45.04 | 46.54 | +1.50 ns |
| mask (fast) | 45.08 | 45.85 | +0.77 ns |
| nonmask (bag) | 44.92 | 49.67 | +4.75 ns |
| accessor | 44.99 | 50.73 | +5.74 ns |

Low single-digit ns, 2–13% of an op that does nothing else — and every real
permission check does considerably more. [D2](https://github.com/bartlomieju/wayfinder/issues/17)'s
cell is not a cost problem, and the comment on this ticket asking whether
accessors "can carry the bracket at all" is answered yes: the accessor returns
an ordinary value, and the bracket lives at the op layer, independent of which
installation shape is chosen.

### 6. Three traps in the installation, each a day lost

- **`Object::set_accessor` does not take on the global proxy.** The property
  ends up *absent* rather than accessor-backed, so every free-name read of it
  becomes a `ReferenceError` — which is how `Deno is not defined` blew up ext JS
  mid-bootstrap. `define_property` with a get/set `PropertyDescriptor` works.
- **`Context::get_slot::<T>()` keys on the inner type.** `set_slot` takes an
  `Rc<T>`; asking for `Rc<Bag>` silently returns `None`, which is
  indistinguishable from "the bag is empty".
- **`deno_runtime` is a *build*-dependency of `deno_snapshots`**, so its cargo
  features do not unify with the CLI's normal dependency on it. The interceptor
  has to be enabled in both, or the binary expects a handler the snapshot does
  not have.

### 7. Moving an authority global is observable, and `process` is not covered

Both bag-based shapes have to take the guarded names off the global object.
**Reading them to do so runs JavaScript.** `process`, `WebSocket` and
`navigator` are lazy accessors installed by `core.createLazyLoader`, so a naive
harvest forces the whole node bootstrap that deno's node-defer work exists to
avoid — and it deadlocks: reading `process` runs `node:process`, which reads
free-name `Deno`, which the harvest has already deleted.

The prototype skips lazily-defined names and reports them:

```
[permcap] bagged ["Deno", "fetch"] deferred(lazy)["WebSocket", "navigator", "process"]
```

So the guarded-read numbers above are for `fetch`, and **`process` is not
covered by either bag-based shape as built** — while being the single most-read
guarded global in the express graph (1069 of 1117 guarded reads, P1 finding 3).

Worse, this is not just a harvest-ordering bug. A global defined *after* the
install point is covered by neither shape: an accessor gets overwritten when
node bootstrap defines the real `process`, and NON_MASKING stops firing once the
name is present. Covering late-defined globals means hooking the definition site
— in bootstrap, the way ext/node populated `__bootstrap.ext_node_nodeGlobals` —
rather than wrapping after the fact. That is a spec question this cut does not
answer.

## Recommendation to the spec

**Per-property accessors on the live global, installed from bootstrap.** They
are at baseline for every unrelated read, they are the only shape whose presence
is a runtime decision (which D4 requires), they need no external reference and
no snapshot participation, and the guarded-read cost is the same as the
alternative. The masking interceptor P1 built should be rejected outright. The
NON_MASKING interceptor is a viable fallback and is the shape with production
precedent, but it costs 6.66 ns on every `globalThis.X` read for no benefit
accessors do not already provide.

## What this prototype does not answer

- Coverage of globals defined after the install point (`process` above).
- Whether the ~270 ns guarded read can be brought down, and whether it needs to.
- Channel 2 in any form.
- Worker contexts: the install runs in `bootstrapMainRuntime` only.
