# Dedicated-allocator opportunities: deno / rusty_v8 / deno_graph

Analysis-only shortlist (2026-07-16). Line numbers approximate against: deno @
main (803a3c933e), rusty_v8 @ v150.2.0 (d305e6a), deno_graph @ 7b484e12.
Verification of actual impact is deliberately left to the reader.

## Cross-cutting themes

1. **Specifier/Url interning** — `ModuleSpecifier = url::Url` is a heap-String
   wrapper with no cheap clone. It is cloned per dependency edge in deno_graph
   and per resolve in cli/resolver. An interning layer (arena + u32 ids, or
   Arc<Url>) collapses a dozen findings at once.
2. **Missing fast hashers** — no FxHash/ahash anywhere in deno_graph src/ or
   runtime/permissions|ops. Meanwhile libs/npm hashes `NodeId(u32)` with
   SipHash. `rustc-hash` is already a workspace dep in cli/libs — drop-in.
   Systemic one-line fix available: `MaybeDashMap` default `S = RandomState`
   (libs/maybe_sync/lib.rs:39,83).
3. **Per-I/O-op permission-check allocations** — every fs/net op pays parses,
   case-folds, and format! strings that are dropped unused on the granted path.
4. **Per-request/per-chunk HTTP allocations** — header name/value double
   allocations, URL assembly via format!, full-chunk copies on body paths.
5. **V8 boundary string crossings** — every V8→Rust string materializes a fresh
   `String`; serializer buffers malloc per postMessage.
6. **Phase-scoped object graphs → arenas** — npm resolution nodes (existing
   maintainer TODO), deno_graph symbols/fast_check strings.

---

## Payoff vs effort

Effort: S = hours, localized, no API change; M = days, several files, internal
API only; L = weeks, public-API ripple across crates/repos. Payoff: expected
impact on a realistic workload (serve RPS, check/install wall-time, fs/net op
throughput), assuming the finding verifies.

### Tier A — quick wins (S effort, do first)

| Candidate                                                                                                                                            | Payoff          | Effort | Notes                                                                                                                             |
| ---------------------------------------------------------------------------------------------------------------------------------------------------- | --------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------- |
| Hasher sweep: node_resolver caches, libs/npm NodeId maps, MaybeDashMap default, deno_graph internal maps, CjsTracker/type_checker/module_loader sets | med-high        | S      | Mechanical, internal-only, near-zero risk. MaybeDashMap default is one line. SipHash over PathBuf/Url keys is the worst offender. |
| Permissions: lazy audit value + single host parse (runtime #1+#2)                                                                                    | high            | S      | Macro takes a closure; reuse the parsed descriptor. Every net op wins; clearly redundant work today.                              |
| Permissions: comparison_path single-pass case-fold (runtime #3)                                                                                      | high (macOS fs) | S-M    | Drop the uppercase→lowercase double alloc; scratch buffer. Must preserve documented Unicode-fold semantics.                       |
| op_http_get_request_url scratch string (ext #2)                                                                                                      | med             | S      | Per request; thread-local String.                                                                                                 |
| request_body BytesMut::freeze + op_fetch_send with_capacity (ext #9, #5)                                                                             | small-med       | S      | Trivial, contained.                                                                                                               |
| resolve_with_graph: drop format!("npm:{}") (cli #7)                                                                                                  | small           | S      | Parse from raw specifier.                                                                                                         |

### Tier B — high payoff, moderate effort (M)

| Candidate                                                                     | Payoff               | Effort | Notes                                                                                                 |
| ----------------------------------------------------------------------------- | -------------------- | ------ | ----------------------------------------------------------------------------------------------------- |
| npm solver node/path arena (cli #2)                                           | high (install)       | M      | Phase-scoped, self-contained crate, maintainer TODO already blesses it.                               |
| Body-chunk zero-copy: raw H1 read + fetch upload (ext #3, #4)                 | high (serve/upload)  | M      | Bytes slice of scratch; lifetime care needed. BYOB variant exists as template.                        |
| Response-header interning + bump buffer (ext #1)                              | high (serve)         | M      | Request side already does the bump-buffer pattern — port it.                                          |
| RawHttpRecord pool (ext #8)                                                   | med-high (raw serve) | M      | HttpServerState::pool is the in-tree template.                                                        |
| rusty_v8 scratch-buffer string variant + migrate hot callers (rusty_v8 #1-#3) | very high ceiling    | M+     | Adding the API is S-M; payoff only lands as deno_core/deno callers migrate (cross-repo, incremental). |
| ValueSerializer pooled buffer (rusty_v8 #4)                                   | med (workers)        | M      | Contained to the delegate; matters for postMessage-heavy code.                                        |

### Tier C — big bets (L effort, verify payoff first)

| Candidate                                                                              | Payoff                     | Effort | Notes                                                                                                  |
| -------------------------------------------------------------------------------------- | -------------------------- | ------ | ------------------------------------------------------------------------------------------------------ |
| deno_graph specifier interning: Arc<Url>/ids in Range + Dependency (graph #1, #3, #10) | highest in graph workloads | M-L    | Public-API ripple into cli, deno_doc, deno_lint consumers. Arc<Url> variant is the cheaper first step. |
| deno_graph id-indexed graph storage (graph #2)                                         | high                       | L      | Overlaps the above; do after interning proves out. symbols module is the template.                     |
| Permission descriptor Vec → indexed structure (runtime #6)                             | conditional                | M-L    | Only pays with many allow/deny entries; precedence semantics are the risk.                             |

### Tier D — low priority / only if profiling says so

Cookie join, remote_addr format!, request-target copy, websockets hasher,
require path walks (ext #11-#15); sys/blind/run descriptor allocs (runtime
#8-#10); disk-cache filename, lockfile strings, exports PathBuf churn (cli
#11-#13); Weak/Context boxes, BackingStore pool, FinalizerMap hasher, CDP
buffers — inspector-only (rusty_v8 #5-#10); deno_graph symbols/fast_check string
work (graph #7-#9) unless fast_check shows up in profiles.

---

## deno_graph (module graph — per-module/per-edge costs on every run/check/install)

1. **`Range` embeds a full `Url` cloned per import + per resolution** — HIGH
   src/graph.rs:226-233 (struct); built/cloned at ~3516/3535/3559/3590/3675/
   3709/3757/3922/3953/4020/4076 and re-cloned at 2070/2956/3024-3093/3524/… The
   specifier is always the referrer module's URL — same value repeated for every
   import and 2x per edge (code+type resolve). Fix: `Arc<Url>` or interned u32
   referrer id. Single highest-leverage change in deno_graph.
2. **Graph storage `BTreeMap<Url, ModuleSlot>` + `BTreeMap<Url, Url>`
   redirects** — HIGH src/graph.rs:2190-2197; lookups 2057-2059, 3107, 3133.
   O(log n) full-URL string comparisons per insert/lookup/redirect. Fix:
   interned ids + id-indexed Vec/slab (src/symbols already does exactly this
   with ModuleId(u32)).
3. **`Resolution`/`Dependency`/`Import` own 4-6 heap allocs per edge** — HIGH
   src/graph.rs:871-883, 1052-1068, 1033-1048. Interning fixes.
4. **Default SipHash on all internal maps** — MED-HIGH No fast hasher in src/ at
   all. Hot maps: graph.rs:4588, 4592, 6658-6668; ast/mod.rs:114;
   symbols/collections.rs:73; fast_check/range_finder.rs:233.
5. **`IndexMap.entry(import.specifier.clone())` per import** — MED-HIGH
   src/graph.rs:4059 (+3557/3610/3673/3754). Clones key even on hit; interning
   or hashbrown entry_ref.
6. **SWC `Atom` → owned `String` per import in analysis glue** — MED
   src/ast/mod.rs:442,460,469 — throws away the interned Atom.
7. **Per-symbol `.text_fast().to_string()` (~30 sites)** — MED
   src/symbols/analyzer.rs:376-458. Borrow from Arc'd source or arena.
8. **`DefinitionPathNode { parts: Vec<String> }` cloned per trace branch** — MED
   src/symbols/cross_module.rs:389-679. SmallVec<[Cow;2]> + interning.
9. **fast_check range_finder: String-keyed export maps +
   `traced_exports.clone()`** — MED-LOW
   src/fast_check/range_finder.rs:42,48,120,190,201,233,244,251.
10. **npm dep re-resolution clones `Range` ~4x per npm edge** — MED
    src/graph.rs:3012-3103. Falls out of #1.
11. **`format!` in jsx-import-source / dynamic template specifiers** — LOW-MED
    src/graph.rs:3670,3705,4309.
12. **`AttributeTypeWithRange { kind: String }`** — LOW — graph.rs:4506-4510;
    kind is a tiny closed set ("json"…), ideal interning target.
13. **`DefaultParsedSourceStore` Url-key clones + default hasher** — LOW-MED
    src/ast/mod.rs:114,151-152,209-213.

## deno cli/ + libs (per-module costs on check/install/startup)

1. **npm solver: SipHash maps keyed on `NodeId(u32)`** — HIGH
   libs/npm/resolution/graph.rs:84,151-152,304-363(tarjan),384-400,538-587,
   634,703,806,909,1017-1018,1104,1650-1651. FxHash drop-in; highest leverage.
2. **npm solver node/path arena — existing maintainer TODO** — HIGH
   libs/npm/resolution/graph.rs:58 has a
   `// todo(dsherret): ... arena/bump
   allocator` comment. Node struct ~:90
   holds three BTreeMaps; thousands of phase-scoped nodes. Textbook bumpalo
   case.
3. **node_resolver thread-local `HashMap<PathBuf,_>` caches use SipHash** — HIGH
   libs/node_resolver/cache.rs:33-36 (CANONICALIZED_CACHE, FILE_TYPE_CACHE). Hit
   per path segment of every resolution; SipHash over long paths.
4. **`ParsedSourceCache` DashMap uses RandomState** — MED-HIGH
   libs/resolver/cache/parsed_source.rs:59; per-module insert + lookups.
5. **`MaybeDashMap` default `S = RandomState` (systemic)** — MED-HIGH
   libs/maybe_sync/lib.rs:39,83 — single edit fixes all resolver DashMaps.
6. **`CjsTracker.known` default hasher + `require_modules` O(n) Vec::contains**
   — MED libs/resolver/cjs/mod.rs:30,95,154,267,294,301.
7. **`resolve_with_graph`: per-edge Url clone + `format!("npm:{}")`** — MED
   libs/resolver/graph.rs:323-326,434.
8. **type_checker: SipHash `seen` sets + `format!` diagnostic keys** — MED
   cli/type_checker.rs:883,930-935,503,523-531.
9. **module_loader `loaded_files: HashSet<ModuleSpecifier>` SipHash + clone per
   load** — MED cli/module_loader.rs:596,1354,1075.
10. **npm snapshot maps SipHash + id clones in dedup loop** — MED
    libs/npm/resolution/snapshot.rs:122-135,219-283,348-377.
11. **node exports resolution PathBuf churn per candidate** — MED-LOW
    libs/node_resolver/resolution.rs:649,691,731,1064,1415-1535,1445,1551.
12. **disk cache filename format!/replace per file** — LOW
    libs/resolver/cache/disk_cache.rs:49,55-103.
13. **lockfile redirect/mapping to_string loops** — LOW
    cli/graph_util.rs:1043-1064. Excluded (already good): emit cache uses
    XxHash64; type_checker uses FastInsecureHasher; graph.clone() sites already
    todo(perf)'d, arena-hostile.

## deno runtime/ (per-I/O-op permission checks; hottest sync path in the runtime)

1. **Eager audit-value construction on every net/fs check** — HIGH
   runtime/permissions/lib.rs:145-153 (macro), check_net ~4613-4621, vsock
   ~4652-4656, unix ~4681-4685. Host parse + display format! built and dropped
   even when AUDIT_SINK unset (lib.rs:88-94). Fix: closure-lazy value.
2. **check_net parses the host TWICE** — HIGH
   runtime/permissions/lib.rs:4606-4626 — FQDN parse + NetDescriptor built once
   in the audit block, again for inner.check. Every connect/listen/DNS op.
3. **comparison_path: ~4 heap allocs per path per fs op (macOS)** — HIGH
   runtime/permissions/lib.rs:1330-1340 (nfkd→String→to_uppercase→to_lowercase→
   PathBuf), 1326-1328 (Windows), via PathQueryDescriptor::new :1405,1427.
   Single-pass case-fold, thread-local scratch, or LRU path→cmp_path cache.
4. **check_open: Cow<Path> clone + re-normalize per op** — HIGH
   runtime/permissions/lib.rs:4204 (+4301,4339,4702,4735), normalize_path
   :1395/1401. Pass Cow by move; scratch buffers.
5. **NetDescriptor::display_name always format!s** — MED-HIGH
   runtime/permissions/lib.rs:1921-1923 (contrast PathQueryDescriptor :1447-1452
   which returns Cow::Borrowed).
6. **Per-op descriptor set is a linearly scanned Vec** — MED
   runtime/permissions/lib.rs:852-857, scan at 1054-1085. Exact-match hash set +
   prefix structure; must preserve precedence semantics (841-848).
7. **Default SipHash int-keyed maps** — MED-LOW
   runtime/ops/worker_host.rs:129,149-150 (THREAD_REGISTRY, hot on
   postMessageToThread); tty.rs:41; fs_events.rs:230; desktop.rs:278.
8. **SysDescriptor::parse allocates String from closed kind set per sys op** —
   LOW runtime/permissions/lib.rs:2951; runtime_descriptor_parser.rs:114.
9. **check_open_blind format!("<{}>") even when check passes** — LOW-MED
   runtime/permissions/lib.rs:4207.
10. **format_display_name / RunQueryDescriptor::parse** — LOW lib.rs:744-750;
    lib.rs:2589 (spawn is heavyweight anyway). Dead ends verified: log closures
    already lazy behind DEBUG_LOG_ENABLED; fmt_errors is cold; worker msg
    buffers live in ext/.

## deno ext/ (per-request / per-chunk HTTP + fetch)

Already-optimized (skip): HttpRecord pool + HeaderMap reuse
(ext/http/service.rs), stream_resource ring buffer, response_body gzip buffer
recycling.

1. **Response headers: Cow→Vec double alloc + HeaderName::from_bytes re-parse
   per header** — HIGH ext/http/http_next.rs:2598 (append_response_header) +
   callers 2586,2614, 3277,3299; hyper path re-parse at service.rs-side :200;
   raw path RawHeader = two Vec<u8> (:368). Fix: static-intern common names;
   bump-allocate header bytes into one per-request buffer (request side already
   does this, RawRequestHeaders::bytes :437-512).
2. **op_http_get_request_url: format! temporaries + fresh String per request** —
   HIGH ext/http/http_next.rs:2075-2141 (2096, 2108, 2120, 2126). Thread-local
   scratch.
3. **fetch upload: full chunk copy + boxed read future per chunk** — HIGH
   ext/fetch/lib.rs:375-401 (392-393: `buf.to_vec().into()` + re-boxed read).
   Zero-copy BufView→Bytes; owned/reused future.
4. **Raw H1 body read copies each chunk (`chunk.to_vec()`)** — HIGH
   ext/http/http_next.rs:983-998 (:993). Bytes slice of scratch (BYOB variant
   already exists :1000).
5. **op_fetch_send header vec: no with_capacity, owned copy per header** —
   MED-HIGH ext/fetch/lib.rs:666-669.
6. **op_fetch re-parses every outbound header (HeaderName/Value::from_bytes)** —
   MED ext/fetch/lib.rs:524-531. Static-intern common names.
7. **Raw compression header rewrite: several Vec allocs + to_lowercase per
   response** — MED ext/http/http_next.rs:922-972.
8. **RawHttpRecord not pooled (unlike hyper-path HttpRecord)** — MED
   ext/http/http_next.rs:1158-1243, Rc::new per request :1218, take :1409-1411.
   Give raw path the same freelist treatment.
9. **request_body try_take_full: BytesMut→Vec copy on small-body fast path** —
   MED ext/http/request_body.rs:70,83. BytesMut::freeze instead.
10. **Boxed future per body read (Resource::read idiom)** — MED
    ext/http/http_next.rs:1685-1693 (+2550-2584).
11. **Cookie join: Vec + joined buffer per request with cookies** — MED-LOW
    ext/http/http_next.rs:2371-2420, 2432-2483 (TODO at :2466 suggests JS-side).
12. **remote_addr: format! + parse per access** — MED-LOW
    ext/http/http_next.rs:2242,2287 (peer_address Rc<str> already exists —
    reuse).
13. **Raw path always heap-copies request target String** — MED-LOW
    ext/http/http_next.rs:398, 3334-3338.
14. **ActiveWebSockets HashMap<u64,_> default hasher** — LOW — service.rs:117.
15. **require resolution Vec<String> path walks** — LOW (first-load only)
    ext/node/ops/require.rs:174-220,360-399 + to_string_lossy sites.

## rusty_v8 (JS↔Rust boundary; v150.2.0)

Already-optimized (verified non-issues): callbacks are compile-time extern "C"
trampolines (no per-call Box); argument arrays use zero-cost slice_into_raw;
isolate/context slots already use custom BuildTypeIdHasher; V8 tasks are opaque
C++ ptrs; inspector StringView borrows.

1. **`String::to_rust_string_lossy` allocates a fresh String per crossing** —
   HIGH src/string.rs:970-989. THE hottest boundary. `to_rust_cow_lossy` (:1054)
   and `write_utf8_into`-style reuse (:1000) already exist — push callers there,
   or add thread-local scratch variant.
2. **latin1/wtf16 transcode: Vec::with_capacity per non-ASCII conversion** —
   HIGH src/string.rs:1247-1272, 1277-1302. Thread-local transcode buffer.
3. **`Value::to_rust_string_lossy` funnels through #1** — HIGH
   src/value.rs:566-585.
4. **ValueSerializer buffer: raw alloc/realloc per
   structured-clone/postMessage** — MED-HIGH src/value_serializer.rs:171-208,
   handed off as Vec :556-575. Pooled buffer.
5. **Weak::new_raw Box<WeakData> + Box<dyn FnOnce> finalizers** — MED
   src/handle.rs:745-749, 784-800, 1095-1096. Slab per isolate.
6. **CDP Dispatchable accessors: zeroed Vec + copy + double alloc per inspector
   msg** — MED src/crdtp.rs:220-247, 355-361 (method_str re-allocates :229-231).
7. **BackingStore copy-based creation paths** — MED
   src/shared_array_buffer.rs:171-202 + array_buffer parallels. Pooled
   fixed-size backing stores for small-buffer case.
8. **Per-Context Box<ContextAnnex> + boxed finalizer** — MED
   src/context.rs:186-223. Matters for worker/realm/vm-heavy workloads.
9. **FinalizerMap: SipHash HashMap keyed on FinalizerId int** — MED
   src/handle.rs:1101. Identity hasher or slab.
10. **Per-isolate annex/liveness boxes, startup CStrings** — LOW (cold).
    src/isolate.rs:1015,2071; src/V8.rs:128,141; src/icu.rs:68.
