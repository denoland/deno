# NPM Install Performance Analysis

## Benchmark (astro project, cold cache)

| Tool | Wall (mean) | User | System |
|------|-------------|------|--------|
| Bun | 1.82s | 0.44s | 8.43s |
| Deno (modified) | 5.16s | 1.93s | 6.68s |
| Deno (stock) | 9.72s | 2.33s | 7.17s |

## Phase Breakdown (Deno modified, 3 runs)

| Phase | Run 1 | Run 2 | Run 3 |
|-------|-------|-------|-------|
| Resolution | 2.90s | 3.23s | 2.54s |
| Tarball DL+extract+clone | 1.44s | 1.22s | 1.41s |
| Symlinks + setup | 0.05s | 0.06s | 0.04s |
| sync_resolution_with_fs | 1.50s | 1.28s | 1.45s |

Resolution is 60-70% of total time.

## Flamegraph Summary (samply profile)

Total samples: 2263 | ~2263ms

| Category | % of total | ~Time | Notes |
|----------|-----------|-------|-------|
| kevent (idle/waiting) | 65.4% | 1479ms | Waiting for network I/O |
| Gzip inflate + CRC32 | 12.1% | 274ms | On main async thread, blocks event loop |
| Byte assembly (into_iter) | 5.0% | 113ms | Byte-by-byte copy, blocks event loop |
| H2/TLS I/O processing | 5.7% | 129ms | rustls decrypt, h2 framing |
| node_modules symlinks | 2.0% | 45ms | symlink_package_dir |
| Memory cache drop | 1.5% | 34ms | Dropping NpmPackageInfo maps |
| Resolution graph logic | 1.4% | 32ms | resolve_pending traversal |

### Key Call Tree Paths

- `download_with_retries_on_any_tokio_runtime` → 21.8% of total
  - `inflate` (gzip decompress): 9.2%
  - `crc32fast`: 2.9%
  - `Vec::extend_desugared` (byte assembly): 5.0%
  - `Collected::to_bytes`: 1.5%
- `sync_resolution_with_fs` → 2.6%
  - `symlink_package_dir`: 2.0%
- `clear_memory_cache` → 1.5% (dropping Arc<NpmPackageInfo>)
- `resolve_pending` → 1.1%

## Resolution Critical Path (astro@5.17.3)

1. Fetch `astro` packument (8.6MB, 57-65ms parse)
2. Fetch 64 child manifests via FuturesOrdered
   - First manifest ready: 101-226ms
   - All manifests ready: 1.0-1.2s
   - Big ones on critical path: vite (37.8MB, 44ms parse), typescript (14.9MB, 66-79ms), @types/node (10.6MB, 39-49ms), rollup (5.9MB, 28ms)
3. Second wave: unstorage (27 deps, 150-334ms), sharp (27 deps, 145-160ms)
4. Total resolve_pending: 2.17-2.95s across 3552 parents in 4 batches

## Biggest Packuments by Size

| Package | Size | Parse Time | Save Time |
|---------|------|------------|-----------|
| vite | 37.8MB | 44ms | 2.8-9.7ms |
| typescript | 14.9MB | 66-79ms | 7.5-7.8ms |
| @types/node | 10.6MB | 39-49ms | 2.0-2.2ms |
| astro | 8.6MB | 57-65ms | 8.6-9.8ms |
| @azure/cosmos | 6.7MB | 25-31ms | 4.5-6.4ms |
| rollup | 5.9MB | 27-29ms | 4.8-5.3ms |
| @azure/identity | 5.8MB | 20-25ms | 3.6-4.3ms |

## Tarball Phase Details

278 packages total. Biggest tarballs:

| Package | Size | Download | Extract |
|---------|------|----------|---------|
| @img/sharp-libvips-darwin-arm64 | 7.5MB | ~540ms | ~860ms |
| typescript | 4.3MB | ~450-550ms | ~655-965ms |
| @esbuild/darwin-arm64@0.27.3 | 4.3MB | ~450-550ms | ~800-823ms |
| @astrojs/compiler | 1.4MB | ~310-365ms | ~672-1093ms |
| @shikijs/langs | 1.2MB | ~260-400ms | ~768-1031ms |

## Actionable Improvements

### 1. Fix byte-by-byte copy (quick, ~5% = ~100-150ms)
**File**: `cli/http_util.rs:377`
**Current**: `data.extend(bytes.into_iter())` — iterates byte-by-byte
**Fix**: `data.extend_from_slice(&bytes)` — uses memcpy

### 2. Move gzip decompression off main thread (~12% = ~200-400ms)
Currently `tower-http` transparent decompression runs inline on the single-threaded
async runtime, blocking event loop from servicing other HTTP/2 streams.
**Options**:
- Receive raw compressed bytes, decompress on `spawn_blocking`
- Or disable Accept-Encoding for packument requests (bandwidth tradeoff)

### 3. Defer clear_memory_cache drop (~1.5% = ~30-50ms)
`clear_memory_cache` at end of resolution drops huge NpmPackageInfo hashmaps.
Move to `spawn_blocking` so it doesn't block transition to tarball phase.

### 4. Investigate: more concurrent packument requests
65% idle time suggests we could benefit from more overlapping network requests.
The serial BFS resolution limits this — each depth level waits for the previous.

## Registry Stats (typical run)

- cache_hits: 3962
- pending_awaits: 386
- network_fetches: 370
- peak_in_flight: 64
- prefetch_calls: 6614
- prefetch_already_cached: 6270
- prefetch_skipped_at_capacity: 129-133

---

## Bun Architecture Analysis (from source code review)

### Key Architectural Differences

#### 1. Event-driven pipeline (not sequential phases)

Bun does NOT have separate sequential phases. Instead, it uses an **event-driven loop**
where resolution, tarball download, and extraction all happen concurrently:

```
Main thread event loop (runTasks):
  1. Pop completed network tasks from async_network_task_queue
  2. For each completed manifest download:
     → Parse manifest on thread pool (enqueueParseNPMPackage)
  3. For each parsed manifest (from resolve_tasks queue):
     → Insert into manifests map
     → Immediately call processDependencyList() — resolves deps, enqueues
       child manifest fetches AND tarball downloads in the same pass
  4. For each completed tarball download:
     → Enqueue extraction to thread pool (enqueueExtractNPMPackage)
  5. Schedule all accumulated batches:
     → Thread pool: parse tasks + extract tasks
     → HTTP thread: manifest requests + tarball requests
  6. Repeat until pending_tasks == 0
```

This means: while `vite`'s 37MB manifest is downloading, other manifests that
already arrived are being parsed on the thread pool, their deps are being
resolved, and tarballs for already-resolved packages are downloading.

**Deno's approach**: Strictly sequential. `resolve_pending()` must fully complete
(all 370 packument fetches) before ANY tarball download or node_modules work starts.
The tarball "prefetching" during resolution helps, but the node_modules phase
still waits for resolution to finish completely.

#### 2. Manifest parsing on thread pool (not main thread)

Bun: `enqueueParseNPMPackage()` → pushed to `task_batch` → thread pool.
The main thread never blocks on JSON parsing. Results come back via `resolve_tasks` queue.

Deno: `serde_json::from_slice()` happens inside `create_load_future()` via
`spawn_blocking`, which is good, BUT the gzip decompression happens inline
on the main async thread before the bytes even reach `spawn_blocking`.

#### 3. 64 concurrent HTTP requests (configurable)

Bun defaults to 64 max simultaneous HTTP requests (`max_simultaneous_requests`).
Adaptive: halves on network errors, configurable via `--network-concurrency`.

Deno: No explicit HTTP concurrency limit for registry requests (limited by
prefetch concurrency at 50), but the serial BFS resolution means in practice
fewer requests are in flight at any time.

#### 4. clonefile on macOS (same approach as Deno)

Bun's default on macOS is `clonefile()` — a single syscall that creates a
copy-on-write clone of an entire directory tree. This is nearly instant on APFS.
Falls back to `clonefile_each_dir` → `hardlink` → `copyfile`.

Deno: `clone_dir_recursive()` ALSO uses clonefile first, falling back to
recursive copy only if it fails (e.g. destination already exists).
**Same approach — not a differentiator.**

#### 5. libdeflate for fast gzip decompression

Bun uses libdeflate (a faster alternative to zlib) for decompressing HTTP responses.
It reads the gzip footer to get the uncompressed size, pre-allocates the exact
buffer, and does a single-pass decompression. Falls back to zlib for streaming.

Deno: Uses `flate2` (zlib wrapper) via `async_compression` crate, streaming
decompression on the main async thread.

#### 6. Custom binary manifest cache format

Bun serializes parsed manifests to a custom binary format on disk
(`PackageManifest.Serializer.saveAsync`). On subsequent installs, it loads
the binary format instead of re-parsing JSON. This avoids the 44-79ms JSON
parse times for large packuments.

Deno: Caches the raw JSON to disk and re-parses with `serde_json` each time.

### Summary: Why Bun is 2.8x Faster

| Factor | Bun | Deno | Impact |
|--------|-----|------|--------|
| Pipeline | Event-driven, all phases overlap | Sequential phases | **HIGH** — eliminates 1-2s |
| node_modules | clonefile | clonefile | **SAME** |
| Gzip decompress | libdeflate, off main thread | flate2, on main async thread | **MEDIUM** — unblocks event loop |
| HTTP concurrency | 64 concurrent, dedicated thread | ~50 prefetch limit, shared runtime | **MEDIUM** |
| Manifest parse | Thread pool | spawn_blocking (but gzip inline) | **LOW-MEDIUM** |
| Byte collection | Pre-allocated from gzip footer | byte-by-byte into_iter | **LOW** |
| Manifest cache | Binary format | Re-parse JSON each time | **LOW** (cold cache only) |

### Highest-Impact Changes for Deno

1. **Pipeline resolution + tarball + node_modules** — Don't wait for resolution
   to fully complete before starting node_modules setup. As packages resolve,
   immediately start downloading tarballs AND setting up node_modules. This is
   the biggest architectural change but also the biggest win.

2. **Get gzip decompression off the main thread** — Either use raw bytes and
   decompress on blocking pool, or use a dedicated I/O thread.

3. **Fix extend(bytes.into_iter())** — Quick win, memcpy instead of byte loop.
