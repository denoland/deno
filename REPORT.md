# Op Metrics Unification - Report & Future Improvements

## What was done

Unified the dual codegen path in the `op2` macro. Previously, every op generated
two versions of each dispatch function (one with metrics, one without) and
stored four function pointers in `OpDecl`. Now each op generates a single
dispatch function that checks `opctx.metrics_enabled()` at runtime, and `OpDecl`
stores only two function pointers (`slow_fn` and `fast_fn`).

A `#[op2(no_metrics)]` attribute was added for ops that should never have
metrics instrumentation (avoids both the runtime branch and, for fast ops, the
extra `FastApiCallbackOptions` parameter).

### Files changed

- `libs/ops/op2/config.rs` - Added `no_metrics` flag
- `libs/ops/op2/generator_state.rs` - Removed `slow_function_metrics` /
  `fast_function_metrics`
- `libs/ops/op2/dispatch_slow.rs` - Single wrapper with runtime metrics check
- `libs/ops/op2/dispatch_async.rs` - Same
- `libs/ops/op2/dispatch_fast.rs` - Unified fast function, always includes
  `FastApiCallbackOptions` for metrics access
- `libs/ops/op2/mod.rs` - Removed dual codegen orchestration
- `libs/core/extensions.rs` - Removed `slow_fn_with_metrics` /
  `fast_fn_with_metrics` from `OpDecl`
- `libs/core/ops.rs` - Simplified `OpCtx::new()` and `external_references()`
- `libs/core/runtime/bindings.rs` - Removed metrics-based function pointer
  selection

### Impact

- ~17% reduction in generated code per op (~1270 ops across the codebase)
- Eliminated one `CFunctionInfo` definition per fast op
- Simplified runtime dispatch (no branching on which function pointer to use)
- Negligible runtime overhead: one well-predicted branch per op call

---

## Future improvements

### 1. Inline metrics into `slow_function_impl` (avoid double OpCtx extraction)

Currently, for non-`no_metrics` ops, `v8_fn_ptr` extracts `OpCtx` for the
metrics check, then calls `slow_function_impl` which extracts `OpCtx` again
(most ops need it for opstate, isolate, etc.). Move the metrics dispatch inside
`slow_function_impl` itself, after the existing `with_opctx` block. For the
majority of ops that already need `OpCtx`, this eliminates the wrapper overhead
entirely -- `v8_fn_ptr` becomes the trivial 3-line version again, and
`slow_function_impl` handles its own metrics.

### 2. Replace `Rc<dyn Fn>` with direct counter updates

`OpMetricsFn = Rc<dyn Fn(&OpCtx, OpMetricsEvent, OpMetricsSource)>` means every
metrics call goes through Rc ref + vtable dispatch. For the common case (summary
counting), the generated code could directly increment a counter via a pointer
stored in `OpCtx`:

```rust
// Instead of:
(opctx.metrics_fn.as_ref().unwrap_unchecked())(opctx, event, source);

// Do:
(*opctx.metrics_counter).ops_dispatched_sync += 1;
```

Store `Option<*mut OpMetricsSummary>` in `OpCtx` for direct counter access. The
`trace_ops` (stderr logging) feature is rare and could use a separate hook.

### 3. Combine Dispatched + Completed for sync ops

For sync ops, we always call `Dispatched` before and `Completed`/`Error` after.
Since they complete synchronously, we could do a single post-op metrics call
that increments both counters, cutting the metrics overhead in half for sync
ops.

### 4. Mark `no_metrics` on internal/hot ops

Ops like `op_leak_tracing_enable`, `op_leak_tracing_submit`, and other internal
infrastructure ops don't need metrics tracking. Marking them
`#[op2(fast, no_metrics)]` avoids unnecessary overhead.

### 5. Consider whether `OpMetricsSummaryTracker` is still needed

Summary metrics are enabled for `test`, `repl`, and `jupyter`
(`cli/args/mod.rs:980-988`). But the op sanitizers use `RuntimeActivityStats`
(tracking in-flight ops by PromiseId), not `OpMetricsSummary`. The
`OpMetricsSummary` feeds `Deno.metrics()` which is an old/deprecated API. If it
can be removed, the entire `OpMetricsSummaryTracker` and `OpMetricsFn` callback
system could be eliminated.
