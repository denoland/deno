# Startup function ordering

Deno release CI arranges startup functions close together in the final
executable. The generated linker order is specific to one binary and is created
during that binary's release job.

Supported release targets:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `aarch64-apple-darwin`

Other targets and non-release profiles use the standard linker configuration.

## Files

| File                                   | Purpose                                                             |
| -------------------------------------- | ------------------------------------------------------------------- |
| `generate_linux_function_orderfile.ts` | Runs the Linux trace workloads and writes an LLD symbol order.      |
| `orderfile_function_tracer_linux.c`    | Records exact Linux function entries with `INT3` or `BRK`.          |
| `generate_macos_function_orderfile.ts` | Runs the macOS trace workloads and writes a Mach-O linker order.    |
| `orderfile_function_tracer_macos.c`    | Records exact first function entries with arm64 `BRK` instructions. |
| `orderfile_trace_runner.c`             | Starts each workload while suspending the generator process.        |
| `verify_orderfile.ts`                  | Compares the baseline and ordered release binaries after linking.   |

The linker integration is implemented in `cli/build.rs`. Release-job
orchestration is defined in `.github/workflows/ci.ts` and materialized in
`.github/workflows/ci.generated.yml`.

CI retains generated orders and reports as workflow artifacts for seven days.

## CI lifecycle

For each supported release target, CI:

1. Builds a baseline release executable with the default linker layout.
2. Copies that executable to `target/release/deno-before-startup-order`.
3. Runs the target-specific function tracer three times for every workload.
4. Writes a linker order from the first-entry sequence.
5. Relinks `deno` with the generated order.
6. Compares symbol coverage and address-order conformance between the two
   executables.
7. Uploads the order and reports before stripping, signing, and packaging.

The trace workloads cover:

- an empty JavaScript module;
- a small TypeScript module;
- an active timer;
- `Deno.serve` plus one request;
- `deno test`;
- `node:crypto`; and
- `deno fmt --check`.

Each generator creates temporary fixtures and a private `DENO_DIR`. The shared
native runner suspends the generator while a workload executes so the
generator's V8 threads cannot affect the trace.

Within one workload, entries from all three traces are combined in first-seen
order. Functions already emitted by an earlier workload are not emitted again.
Aliases at the same address are retained because the linker may expose multiple
names for one function.

## Platform tracing

### Linux x86-64 and arm64

The Linux generator reads defined `STT_FUNC` entries with `readelf` and
linker-visible names with `nm`. The tracer:

- copies executable `PT_LOAD` mappings into a `memfd`;
- maps the copy read-execute at the original addresses;
- keeps a separate read-write alias;
- replaces each selected function's first instruction with x86-64 `INT3` or
  arm64 `BRK`;
- restores the instruction and records the address on its first `SIGTRAP`; and
- synchronizes arm64 data and instruction caches before resuming execution.

The writable alias avoids a writable-executable mapping. Embedded V8 builtin
blob entries are excluded because V8 copies those bytes to a separate executable
mapping.

The resulting order is passed to LLD with `--symbol-ordering-file`.

### macOS arm64

The macOS generator reads `LC_FUNCTION_STARTS` and resolves linker-visible names
with `llvm-nm`. The tracer:

- creates a private shared-memory copy of the executable `__text` mapping;
- keeps separate read-execute and read-write mappings;
- replaces each function entry with an arm64 `BRK`;
- restores the original instruction on its first trap; and
- invalidates the instruction cache before resuming execution.

The resulting order is passed to the Apple linker with `-order_file`.

## Local two-pass build

Build and preserve the baseline release executable:

```sh
unset DENO_USE_STARTUP_ORDER DENO_STARTUP_ORDER_FILE
DENO_SNAPSHOT_MINIFY_SOURCES=1 \
  cargo build --release --locked -p deno --bin deno \
    --features=deno/panic-trace
cp -p target/release/deno target/release/deno-before-startup-order
```

Generate the Linux order:

```sh
TARGET="$(uname -m)-unknown-linux-gnu"
ORDER="$PWD/target/release/startup-order-$TARGET.order"
target/release/deno run -A \
  tools/startup_order/generate_linux_function_orderfile.ts \
  --binary "$PWD/target/release/deno-before-startup-order" \
  --output "$ORDER" \
  --repeats 3 \
  --workload-profile run-first
```

Generate the macOS order:

```sh
ORDER="$PWD/target/release/startup-order-aarch64-apple-darwin.order"
target/release/deno run -A \
  tools/startup_order/generate_macos_function_orderfile.ts \
  --binary "$PWD/target/release/deno-before-startup-order" \
  --output "$ORDER" \
  --repeats 3 \
  --workload-profile run-first
```

Relink `deno`:

```sh
DENO_SNAPSHOT_MINIFY_SOURCES=1 \
DENO_USE_STARTUP_ORDER=1 \
DENO_STARTUP_ORDER_FILE="$ORDER" \
  cargo build --release --locked -p deno --bin deno \
    --features=deno/panic-trace
```

Verify the result before stripping the executable:

```sh
target/release/deno run -A tools/startup_order/verify_orderfile.ts \
  --baseline-binary target/release/deno-before-startup-order \
  --binary target/release/deno \
  --order "$ORDER" \
  --output "$ORDER.verify.json"
```

## Verification

The build integration rejects an empty or implausibly small order. The verifier
then checks:

- the order has no duplicate symbol names;
- at least 90% of its names exist in the baseline executable;
- at least 1,000 names exist in both executables; and
- relinking improves address-order conformance for those common names.

The linked executable may expose fewer names because LTO and identical-code
folding can select different aliases. Those missing names do not fail
verification as long as enough common symbols remain for a meaningful
comparison.

When the baseline executable already follows at least 90% of the requested
sequence, it is treated as already ordered. In that case the final executable
may remain unchanged and must not reduce conformance by more than two percentage
points.

The report records the exact symbol count, missing names, longest nondecreasing
address sequence, and conformance ratio for both executables, plus the direct
comparison over their common symbols. Verification decisions use that
baseline-relative comparison; other symbol counts and performance measurements
remain telemetry.

## Build artifacts

CI retains these files for seven days:

- `startup-order-<target>.order` — linker input;
- `startup-order-<target>.order.json` — workload and trace summary;
- `startup-order-<target>.order.starts.json` — Linux function-discovery summary;
  and
- `startup-order-<target>.order.verify.json` — post-link verification report.

## Reference measurements

The current implementation produced these release-build results:

| Target       | Ordered symbols | Timer-free RSS delta | Empty JS startup delta | Cached TypeScript startup delta |
| ------------ | --------------: | -------------------: | ---------------------: | ------------------------------: |
| Linux x86-64 |          22,963 |          -18,784 KiB |              -1.897 ms |                       -2.009 ms |
| macOS arm64  |          46,869 |          -10,960 KiB |              -1.052 ms |                       -1.078 ms |

These measurements document expected behavior for engineering review. Changes to
a tracer, generator, workload, linker configuration, or verifier should be
evaluated with paired RSS and startup measurements.
