# Plan: Implement proper `resourceLimits` for `node:worker_threads`

## Prerequisite

The `v8` crate needs to be updated with the new `ResourceConstraints` APIs
from https://github.com/denoland/rusty_v8/pull/1918. Once merged and released,
update `v8` version in `/Cargo.toml`.

## Current state

The current implementation in `cli/lib/worker.rs` (lines 373-386) uses
`v8::CreateParams::heap_limits(max_young, max_old)` which calls
`ConfigureDefaultsFromHeapSize(initial, max)`. This is semantically wrong —
it treats `maxYoungGenerationSizeMb` as the initial heap size and
`maxOldGenerationSizeMb` as the total maximum heap size, instead of setting
young/old generation limits independently. `codeRangeSizeMb` and `stackSizeMb`
are accepted but silently ignored.

## Changes needed

### 1. Update `cli/lib/worker.rs` — Use individual constraint setters

Replace the `heap_limits()` call with individual setters. Also read back V8
defaults for unspecified values, matching Node.js behavior
(`node_worker.cc:132-157`).

```rust
let create_params = if let Some(ref limits) = args.resource_limits {
  let mut params = create_isolate_create_params(&shared.sys)
    .unwrap_or_default();

  if let Some(max_old) = limits.max_old_generation_size_mb.filter(|&v| v > 0) {
    params = params.set_max_old_generation_size_in_bytes(max_old * 1024 * 1024);
  }
  if let Some(max_young) = limits.max_young_generation_size_mb.filter(|&v| v > 0) {
    params = params.set_max_young_generation_size_in_bytes(max_young * 1024 * 1024);
  }
  if let Some(code_range) = limits.code_range_size_mb.filter(|&v| v > 0) {
    params = params.set_code_range_size_in_bytes(code_range * 1024 * 1024);
  }

  Some(params)
} else {
  create_isolate_create_params(&shared.sys)
};
```

### 2. Update `runtime/ops/worker_host.rs` — Pass resolved limits back

Add a `ResolvedResourceLimits` struct (with concrete values, not Options) that
gets populated after `CreateParams` is built. This captures what V8 actually
uses (including defaults for unspecified fields). Pass this through
`WorkerMetadata` so the worker's JS `resourceLimits` object reflects real
values.

Node.js does this by reading back from the constraints object:
```cpp
// If user didn't specify, read back V8's default
resource_limits_[kMaxYoungGenerationSizeMb] =
    constraints->max_young_generation_size_in_bytes() / kMB;
```

We should do the same using the new getter APIs on `CreateParams`.

### 3. Handle `stackSizeMb` — Thread stack size + V8 stack limit

Node.js handles `stackSizeMb` in two parts (`node_worker.cc:735-762`):

1. **OS thread stack size**: Set via `uv_thread_create_ex` with
   `thread_options.stack_size = stackSizeMb * MB`.

   In Deno, worker threads are spawned via `std::thread::Builder` at
   `runtime/ops/worker_host.rs:210`. We need to call `.stack_size()`:
   ```rust
   let thread_builder = std::thread::Builder::new()
     .name(format!("{worker_id}"))
     .stack_size(stack_size_bytes);  // from resourceLimits.stackSizeMb
   ```

2. **V8 stack limit**: Inside the worker thread, Node.js computes
   `stack_base = stack_top - (stack_size - kStackBufferSize)` and calls
   `constraints->set_stack_limit(stack_base)`. The stack limit must be set
   from *within* the worker thread (it needs the actual stack pointer).

   This means the `CreateParams` must be built inside the spawned thread,
   or the stack limit must be set after isolate creation. Currently
   `CreateParams` is built in the parent thread inside `create_web_worker_cb`
   which is called from within the spawned thread (`worker_host.rs:236`),
   so this should work — we just need to capture the stack pointer at the
   top of the thread and pass it through.

   The stack buffer size constant in Node.js is `kStackBufferSize = 192 * 1024`.

### 4. Update `ext/node/polyfills/worker_threads.ts` — Worker-side defaults

Currently the worker-side `resourceLimits` is set from metadata at line 803:
```js
resourceLimits = { ...metadata.resourceLimits };
```

Once we pass resolved limits (step 2), the worker will automatically show
correct default values instead of the user-specified values. The key order
should be consistent: `maxYoungGenerationSizeMb`, `maxOldGenerationSizeMb`,
`codeRangeSizeMb`, `stackSizeMb` — matching Node.js.

For the main thread, `resourceLimits` should remain `{}` (line 765).

### 5. Fix `Worker.resourceLimits` on the parent side

Currently the parent-side `Worker.resourceLimits` is just a shallow copy of
what the user passed (`worker_threads.ts:414`). It should instead reflect the
resolved values (with defaults filled in). This could be done by having
`op_create_worker` return the resolved limits, or by computing them in JS
before calling the op.

### 6. Update spec tests

- `tests/specs/node/worker_threads_resource_limits/main.out` — Update expected
  output to match the actual key order from `JSON.stringify(resourceLimits)`.
  The order depends on how the object is constructed in the worker.
- `tests/specs/node/worker_threads_resource_limits/default_main.out` — Update
  to show real V8 defaults instead of `-1` sentinel values.
- `tests/specs/node/worker_threads_resource_limits/oom_main.mjs` — Assert
  `err.code === 'ERR_WORKER_OUT_OF_MEMORY'` and the error message.

### 7. Enable node_compat test

The `test-worker-resource-limits.js` node_compat test uses
`v8.getHeapSpaceStatistics()` which is currently `notImplemented` in
`ext/node/polyfills/v8.ts`. This test cannot pass until that function is
implemented. Either:
- Implement `getHeapSpaceStatistics()` (separate effort), then enable the test
- Or keep it disabled with a reason comment in `tests/node_compat/config.jsonc`

## Files to modify

| File | Change |
|---|---|
| `Cargo.toml` | Bump `v8` crate version |
| `cli/lib/worker.rs` | Use individual constraint setters, handle stack size |
| `runtime/ops/worker_host.rs` | Set thread stack size, pass resolved limits |
| `runtime/web_worker.rs` | Accept resolved limits in options |
| `ext/node/polyfills/worker_threads.ts` | Use resolved limits, fix key order |
| `tests/specs/node/worker_threads_resource_limits/*.out` | Update expected outputs |
| `tests/node_compat/config.jsonc` | Disable test with reason (or enable after implementing `getHeapSpaceStatistics`) |
