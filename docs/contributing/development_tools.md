## Testing and Tools

### Tests

Test `deno`:

```shell
# Run the whole suite:
cargo test

# Only test cli/tests/unit/:
cargo test js_unit_tests
```

Test `std/`:

```shell
cargo test std_tests
```

### Lint and format

Lint the code:

```shell
deno run -A ./tools/lint.js
```

Format the code:

```shell
deno run -A ./tools/format.js
```

### Profiling

To start profiling:

```sh
# Make sure we're only building release.
# Build deno and V8's d8.
ninja -C target/release d8

# Start the program we want to benchmark with --prof
./target/release/deno run tests/http_bench.ts --allow-net --v8-flags=--prof &

# Exercise it.
third_party/wrk/linux/wrk http://localhost:4500/
kill `pgrep deno`
```

V8 will write a file in the current directory that looks like this:
`isolate-0x7fad98242400-v8.log`. To examine this file:

```sh
D8_PATH=target/release/ ./third_party/v8/tools/linux-tick-processor
isolate-0x7fad98242400-v8.log > prof.log
# on macOS, use ./third_party/v8/tools/mac-tick-processor instead
```

`prof.log` will contain information about tick distribution of different calls.

To view the log with Web UI, generate JSON file of the log:

```sh
D8_PATH=target/release/ ./third_party/v8/tools/linux-tick-processor
isolate-0x7fad98242400-v8.log --preprocess > prof.json
```

Open `third_party/v8/tools/profview/index.html` in your browser, and select
`prof.json` to view the distribution graphically.

Useful V8 flags during profiling:

- --prof
- --log-internal-timer-events
- --log-timer-events
- --track-gc
- --log-source-code
- --track-gc-object-stats

To learn more about `d8` and profiling, check out the following links:

- [https://v8.dev/docs/d8](https://v8.dev/docs/d8)
- [https://v8.dev/docs/profile](https://v8.dev/docs/profile)

### Debugging with LLDB

To debug the deno binary, we can use `rust-lldb`. It should come with `rustc`
and is a wrapper around LLDB.

```shell
$ rust-lldb -- ./target/debug/deno run --allow-net tests/http_bench.ts
# On macOS, you might get warnings like
# `ImportError: cannot import name _remove_dead_weakref`
# In that case, use system python by setting PATH, e.g.
# PATH=/System/Library/Frameworks/Python.framework/Versions/2.7/bin:$PATH
(lldb) command script import "/Users/kevinqian/.rustup/toolchains/1.36.0-x86_64-apple-darwin/lib/rustlib/etc/lldb_rust_formatters.py"
(lldb) type summary add --no-value --python-function lldb_rust_formatters.print_val -x ".*" --category Rust
(lldb) type category enable Rust
(lldb) target create "../deno/target/debug/deno"
Current executable set to '../deno/target/debug/deno' (x86_64).
(lldb) settings set -- target.run-args  "tests/http_bench.ts" "--allow-net"
(lldb) b op_start
(lldb) r
```

### V8 flags

V8 has many many internal command-line flags:

```shell
$ deno run --v8-flags=--help _
SSE3=1 SSSE3=1 SSE4_1=1 SSE4_2=1 SAHF=1 AVX=1 FMA3=1 BMI1=1 BMI2=1 LZCNT=1 POPCNT=1 ATOM=0
Synopsis:
  shell [options] [--shell] [<file>...]
  d8 [options] [-e <string>] [--shell] [[--module] <file>...]

  -e        execute a string in V8
  --shell   run an interactive JavaScript shell
  --module  execute a file as a JavaScript module

Note: the --module option is implicitly enabled for *.mjs files.

The following syntax for options is accepted (both '-' and '--' are ok):
  --flag        (bool flags only)
  --no-flag     (bool flags only)
  --flag=value  (non-bool flags only, no spaces around '=')
  --flag value  (non-bool flags only)
  --            (captures all remaining args in JavaScript)

Options:
  --use-strict (enforce strict mode)
        type: bool  default: false
  --es-staging (enable test-worthy harmony features (for internal use only))
        type: bool  default: false
  --harmony (enable all completed harmony features)
        type: bool  default: false
  --harmony-shipping (enable all shipped harmony features)
        type: bool  default: true
  --harmony-regexp-sequence (enable "RegExp Unicode sequence properties" (in progress))
        type: bool  default: false
  --harmony-weak-refs-with-cleanup-some (enable "harmony weak references with FinalizationRegistry.prototype.cleanupSome" (in progress))
        type: bool  default: false
  --harmony-regexp-match-indices (enable "harmony regexp match indices" (in progress))
        type: bool  default: false
  --harmony-top-level-await (enable "harmony top level await")
        type: bool  default: false
  --harmony-namespace-exports (enable "harmony namespace exports (export * as foo from 'bar')")
        type: bool  default: true
  --harmony-sharedarraybuffer (enable "harmony sharedarraybuffer")
        type: bool  default: true
  --harmony-import-meta (enable "harmony import.meta property")
        type: bool  default: true
  --harmony-dynamic-import (enable "harmony dynamic import")
        type: bool  default: true
  --harmony-promise-all-settled (enable "harmony Promise.allSettled")
        type: bool  default: true
  --harmony-promise-any (enable "harmony Promise.any")
        type: bool  default: true
  --harmony-private-methods (enable "harmony private methods in class literals")
        type: bool  default: true
  --harmony-weak-refs (enable "harmony weak references")
        type: bool  default: true
  --harmony-string-replaceall (enable "harmony String.prototype.replaceAll")
        type: bool  default: true
  --harmony-logical-assignment (enable "harmony logical assignment")
        type: bool  default: true
  --lite-mode (enables trade-off of performance for memory savings)
        type: bool  default: false
  --future (Implies all staged features that we want to ship in the not-too-far future)
        type: bool  default: false
  --assert-types (generate runtime type assertions to test the typer)
        type: bool  default: false
  --allocation-site-pretenuring (pretenure with allocation sites)
        type: bool  default: true
  --page-promotion (promote pages based on utilization)
        type: bool  default: true
  --always-promote-young-mc (always promote young objects during mark-compact)
        type: bool  default: true
  --page-promotion-threshold (min percentage of live bytes on a page to enable fast evacuation)
        type: int  default: 70
  --trace-pretenuring (trace pretenuring decisions of HAllocate instructions)
        type: bool  default: false
  --trace-pretenuring-statistics (trace allocation site pretenuring statistics)
        type: bool  default: false
  --track-fields (track fields with only smi values)
        type: bool  default: true
  --track-double-fields (track fields with double values)
        type: bool  default: true
  --track-heap-object-fields (track fields with heap values)
        type: bool  default: true
  --track-computed-fields (track computed boilerplate fields)
        type: bool  default: true
  --track-field-types (track field types)
        type: bool  default: true
  --trace-block-coverage (trace collected block coverage information)
        type: bool  default: false
  --trace-protector-invalidation (trace protector cell invalidations)
        type: bool  default: false
  --feedback-normalization (feed back normalization to constructors)
        type: bool  default: false
  --enable-one-shot-optimization (Enable size optimizations for the code that will only be executed once)
        type: bool  default: false
  --unbox-double-arrays (automatically unbox arrays of doubles)
        type: bool  default: true
  --interrupt-budget (interrupt budget which should be used for the profiler counter)
        type: int  default: 147456
  --jitless (Disable runtime allocation of executable memory.)
        type: bool  default: false
  --use-ic (use inline caching)
        type: bool  default: true
  --budget-for-feedback-vector-allocation (The budget in amount of bytecode executed by a function before we decide to allocate feedback vectors)
        type: int  default: 1024
  --lazy-feedback-allocation (Allocate feedback vectors lazily)
        type: bool  default: true
  --ignition-elide-noneffectful-bytecodes (elide bytecodes which won't have any external effect)
        type: bool  default: true
  --ignition-reo (use ignition register equivalence optimizer)
        type: bool  default: true
  --ignition-filter-expression-positions (filter expression positions before the bytecode pipeline)
        type: bool  default: true
  --ignition-share-named-property-feedback (share feedback slots when loading the same named property from the same object)
        type: bool  default: true
  --print-bytecode (print bytecode generated by ignition interpreter)
        type: bool  default: false
  --enable-lazy-source-positions (skip generating source positions during initial compile but regenerate when actually required)
        type: bool  default: true
  --stress-lazy-source-positions (collect lazy source positions immediately after lazy compile)
        type: bool  default: false
  --print-bytecode-filter (filter for selecting which functions to print bytecode)
        type: string  default: *
  --trace-ignition-codegen (trace the codegen of ignition interpreter bytecode handlers)
        type: bool  default: false
  --trace-ignition-dispatches (traces the dispatches to bytecode handlers by the ignition interpreter)
        type: bool  default: false
  --trace-ignition-dispatches-output-file (the file to which the bytecode handler dispatch table is written (by default, the table is not written to a file))
        type: string  default: nullptr
  --fast-math (faster (but maybe less accurate) math functions)
        type: bool  default: true
  --trace-track-allocation-sites (trace the tracking of allocation sites)
        type: bool  default: false
  --trace-migration (trace object migration)
        type: bool  default: false
  --trace-generalization (trace map generalization)
        type: bool  default: false
  --turboprop (enable experimental turboprop mid-tier compiler.)
        type: bool  default: false
  --concurrent-recompilation (optimizing hot functions asynchronously on a separate thread)
        type: bool  default: true
  --trace-concurrent-recompilation (track concurrent recompilation)
        type: bool  default: false
  --concurrent-recompilation-queue-length (the length of the concurrent compilation queue)
        type: int  default: 8
  --concurrent-recompilation-delay (artificial compilation delay in ms)
        type: int  default: 0
  --block-concurrent-recompilation (block queued jobs until released)
        type: bool  default: false
  --concurrent-inlining (run optimizing compiler's inlining phase on a separate thread)
        type: bool  default: false
  --max-serializer-nesting (maximum levels for nesting child serializers)
        type: int  default: 25
  --trace-heap-broker-verbose (trace the heap broker verbosely (all reports))
        type: bool  default: false
  --trace-heap-broker-memory (trace the heap broker memory (refs analysis and zone numbers))
        type: bool  default: false
  --trace-heap-broker (trace the heap broker (reports on missing data only))
        type: bool  default: false
  --stress-runs (number of stress runs)
        type: int  default: 0
  --deopt-every-n-times (deoptimize every n times a deopt point is passed)
        type: int  default: 0
  --print-deopt-stress (print number of possible deopt points)
        type: bool  default: false
  --opt (use adaptive optimizations)
        type: bool  default: true
  --turbo-sp-frame-access (use stack pointer-relative access to frame wherever possible)
        type: bool  default: false
  --turbo-control-flow-aware-allocation (consider control flow while allocating registers)
        type: bool  default: true
  --turbo-filter (optimization filter for TurboFan compiler)
        type: string  default: *
  --trace-turbo (trace generated TurboFan IR)
        type: bool  default: false
  --trace-turbo-path (directory to dump generated TurboFan IR to)
        type: string  default: nullptr
  --trace-turbo-filter (filter for tracing turbofan compilation)
        type: string  default: *
  --trace-turbo-graph (trace generated TurboFan graphs)
        type: bool  default: false
  --trace-turbo-scheduled (trace TurboFan IR with schedule)
        type: bool  default: false
  --trace-turbo-cfg-file (trace turbo cfg graph (for C1 visualizer) to a given file name)
        type: string  default: nullptr
  --trace-turbo-types (trace TurboFan's types)
        type: bool  default: true
  --trace-turbo-scheduler (trace TurboFan's scheduler)
        type: bool  default: false
  --trace-turbo-reduction (trace TurboFan's various reducers)
        type: bool  default: false
  --trace-turbo-trimming (trace TurboFan's graph trimmer)
        type: bool  default: false
  --trace-turbo-jt (trace TurboFan's jump threading)
        type: bool  default: false
  --trace-turbo-ceq (trace TurboFan's control equivalence)
        type: bool  default: false
  --trace-turbo-loop (trace TurboFan's loop optimizations)
        type: bool  default: false
  --trace-turbo-alloc (trace TurboFan's register allocator)
        type: bool  default: false
  --trace-all-uses (trace all use positions)
        type: bool  default: false
  --trace-representation (trace representation types)
        type: bool  default: false
  --turbo-verify (verify TurboFan graphs at each phase)
        type: bool  default: false
  --turbo-verify-machine-graph (verify TurboFan machine graph before instruction selection)
        type: string  default: nullptr
  --trace-verify-csa (trace code stubs verification)
        type: bool  default: false
  --csa-trap-on-node (trigger break point when a node with given id is created in given stub. The format is: StubName,NodeId)
        type: string  default: nullptr
  --turbo-stats (print TurboFan statistics)
        type: bool  default: false
  --turbo-stats-nvp (print TurboFan statistics in machine-readable format)
        type: bool  default: false
  --turbo-stats-wasm (print TurboFan statistics of wasm compilations)
        type: bool  default: false
  --turbo-splitting (split nodes during scheduling in TurboFan)
        type: bool  default: true
  --function-context-specialization (enable function context specialization in TurboFan)
        type: bool  default: false
  --turbo-inlining (enable inlining in TurboFan)
        type: bool  default: true
  --max-inlined-bytecode-size (maximum size of bytecode for a single inlining)
        type: int  default: 500
  --max-inlined-bytecode-size-cumulative (maximum cumulative size of bytecode considered for inlining)
        type: int  default: 1000
  --max-inlined-bytecode-size-absolute (maximum cumulative size of bytecode considered for inlining)
        type: int  default: 5000
  --reserve-inline-budget-scale-factor (maximum cumulative size of bytecode considered for inlining)
        type: float  default: 1.2
  --max-inlined-bytecode-size-small (maximum size of bytecode considered for small function inlining)
        type: int  default: 30
  --max-optimized-bytecode-size (maximum bytecode size to be considered for optimization; too high values may cause the compiler to hit (release) assertions)
        type: int  default: 61440
  --min-inlining-frequency (minimum frequency for inlining)
        type: float  default: 0.15
  --polymorphic-inlining (polymorphic inlining)
        type: bool  default: true
  --stress-inline (set high thresholds for inlining to inline as much as possible)
        type: bool  default: false
  --trace-turbo-inlining (trace TurboFan inlining)
        type: bool  default: false
  --turbo-inline-array-builtins (inline array builtins in TurboFan code)
        type: bool  default: true
  --use-osr (use on-stack replacement)
        type: bool  default: true
  --trace-osr (trace on-stack replacement)
        type: bool  default: false
  --analyze-environment-liveness (analyze liveness of environment slots and zap dead values)
        type: bool  default: true
  --trace-environment-liveness (trace liveness of local variable slots)
        type: bool  default: false
  --turbo-load-elimination (enable load elimination in TurboFan)
        type: bool  default: true
  --trace-turbo-load-elimination (trace TurboFan load elimination)
        type: bool  default: false
  --turbo-profiling (enable basic block profiling in TurboFan)
        type: bool  default: false
  --turbo-profiling-verbose (enable basic block profiling in TurboFan, and include each function's schedule and disassembly in the output)
        type: bool  default: false
  --turbo-verify-allocation (verify register allocation in TurboFan)
        type: bool  default: false
  --turbo-move-optimization (optimize gap moves in TurboFan)
        type: bool  default: true
  --turbo-jt (enable jump threading in TurboFan)
        type: bool  default: true
  --turbo-loop-peeling (Turbofan loop peeling)
        type: bool  default: true
  --turbo-loop-variable (Turbofan loop variable optimization)
        type: bool  default: true
  --turbo-loop-rotation (Turbofan loop rotation)
        type: bool  default: true
  --turbo-cf-optimization (optimize control flow in TurboFan)
        type: bool  default: true
  --turbo-escape (enable escape analysis)
        type: bool  default: true
  --turbo-allocation-folding (Turbofan allocation folding)
        type: bool  default: true
  --turbo-instruction-scheduling (enable instruction scheduling in TurboFan)
        type: bool  default: false
  --turbo-stress-instruction-scheduling (randomly schedule instructions to stress dependency tracking)
        type: bool  default: false
  --turbo-store-elimination (enable store-store elimination in TurboFan)
        type: bool  default: true
  --trace-store-elimination (trace store elimination)
        type: bool  default: false
  --turbo-rewrite-far-jumps (rewrite far to near jumps (ia32,x64))
        type: bool  default: true
  --stress-gc-during-compilation (simulate GC/compiler thread race related to https://crbug.com/v8/8520)
        type: bool  default: false
  --turbo-fast-api-calls (enable fast API calls from TurboFan)
        type: bool  default: false
  --reuse-opt-code-count (don't discard optimized code for the specified number of deopts.)
        type: int  default: 0
  --turbo-nci (enable experimental native context independent code.)
        type: bool  default: false
  --turbo-nci-as-highest-tier (replace default TF with NCI code as the highest tier for testing purposes.)
        type: bool  default: false
  --print-nci-code (print native context independent code.)
        type: bool  default: false
  --trace-turbo-nci (trace native context independent code.)
        type: bool  default: false
  --turbo-collect-feedback-in-generic-lowering (enable experimental feedback collection in generic lowering.)
        type: bool  default: false
  --optimize-for-size (Enables optimizations which favor memory size over execution speed)
        type: bool  default: false
  --untrusted-code-mitigations (Enable mitigations for executing untrusted code)
        type: bool  default: false
  --expose-wasm (expose wasm interface to JavaScript)
        type: bool  default: true
  --assume-asmjs-origin (force wasm decoder to assume input is internal asm-wasm format)
        type: bool  default: false
  --wasm-num-compilation-tasks (maximum number of parallel compilation tasks for wasm)
        type: int  default: 128
  --wasm-write-protect-code-memory (write protect code memory on the wasm native heap)
        type: bool  default: false
  --wasm-async-compilation (enable actual asynchronous compilation for WebAssembly.compile)
        type: bool  default: true
  --wasm-test-streaming (use streaming compilation instead of async compilation for tests)
        type: bool  default: false
  --wasm-max-mem-pages (maximum initial number of 64KiB memory pages of a wasm instance)
        type: uint  default: 32767
  --wasm-max-mem-pages-growth (maximum number of 64KiB pages a Wasm memory can grow to)
        type: uint  default: 65536
  --wasm-max-table-size (maximum table size of a wasm instance)
        type: uint  default: 10000000
  --wasm-max-code-space (maximum committed code space for wasm (in MB))
        type: uint  default: 1024
  --wasm-tier-up (enable tier up to the optimizing compiler (requires --liftoff to have an effect))
        type: bool  default: true
  --trace-wasm-ast-start (start function for wasm AST trace (inclusive))
        type: int  default: 0
  --trace-wasm-ast-end (end function for wasm AST trace (exclusive))
        type: int  default: 0
  --liftoff (enable Liftoff, the baseline compiler for WebAssembly)
        type: bool  default: true
  --trace-wasm-memory (print all memory updates performed in wasm code)
        type: bool  default: false
  --wasm-tier-mask-for-testing (bitmask of functions to compile with TurboFan instead of Liftoff)
        type: int  default: 0
  --wasm-expose-debug-eval (Expose wasm evaluator support on the CDP)
        type: bool  default: false
  --validate-asm (validate asm.js modules before compiling)
        type: bool  default: true
  --suppress-asm-messages (don't emit asm.js related messages (for golden file testing))
        type: bool  default: false
  --trace-asm-time (log asm.js timing info to the console)
        type: bool  default: false
  --trace-asm-scanner (log tokens encountered by asm.js scanner)
        type: bool  default: false
  --trace-asm-parser (verbose logging of asm.js parse failures)
        type: bool  default: false
  --stress-validate-asm (try to validate everything as asm.js)
        type: bool  default: false
  --dump-wasm-module-path (directory to dump wasm modules to)
        type: string  default: nullptr
  --experimental-wasm-eh (enable prototype exception handling opcodes for wasm)
        type: bool  default: false
  --experimental-wasm-simd (enable prototype SIMD opcodes for wasm)
        type: bool  default: false
  --experimental-wasm-return-call (enable prototype return call opcodes for wasm)
        type: bool  default: false
  --experimental-wasm-compilation-hints (enable prototype compilation hints section for wasm)
        type: bool  default: false
  --experimental-wasm-gc (enable prototype garbage collection for wasm)
        type: bool  default: false
  --experimental-wasm-typed-funcref (enable prototype typed function references for wasm)
        type: bool  default: false
  --experimental-wasm-reftypes (enable prototype reference type opcodes for wasm)
        type: bool  default: false
  --experimental-wasm-threads (enable prototype thread opcodes for wasm)
        type: bool  default: false
  --experimental-wasm-type-reflection (enable prototype wasm type reflection in JS for wasm)
        type: bool  default: false
  --experimental-wasm-bigint (enable prototype JS BigInt support for wasm)
        type: bool  default: true
  --experimental-wasm-bulk-memory (enable prototype bulk memory opcodes for wasm)
        type: bool  default: true
  --experimental-wasm-mv (enable prototype multi-value support for wasm)
        type: bool  default: true
  --wasm-staging (enable staged wasm features)
        type: bool  default: false
  --wasm-opt (enable wasm optimization)
        type: bool  default: false
  --wasm-bounds-checks (enable bounds checks (disable for performance testing only))
        type: bool  default: true
  --wasm-stack-checks (enable stack checks (disable for performance testing only))
        type: bool  default: true
  --wasm-math-intrinsics (intrinsify some Math imports into wasm)
        type: bool  default: true
  --wasm-trap-handler (use signal handlers to catch out of bounds memory access in wasm (currently Linux x86_64 only))
        type: bool  default: true
  --wasm-fuzzer-gen-test (generate a test case when running a wasm fuzzer)
        type: bool  default: false
  --print-wasm-code (Print WebAssembly code)
        type: bool  default: false
  --print-wasm-stub-code (Print WebAssembly stub code)
        type: bool  default: false
  --asm-wasm-lazy-compilation (enable lazy compilation for asm-wasm modules)
        type: bool  default: false
  --wasm-lazy-compilation (enable lazy compilation for all wasm modules)
        type: bool  default: false
  --wasm-lazy-validation (enable lazy validation for lazily compiled wasm functions)
        type: bool  default: false
  --wasm-atomics-on-non-shared-memory (allow atomic operations on non-shared WebAssembly memory)
        type: bool  default: true
  --wasm-grow-shared-memory (allow growing shared WebAssembly memory objects)
        type: bool  default: true
  --wasm-simd-post-mvp (allow experimental SIMD operations for prototyping that are not included in the current proposal)
        type: bool  default: false
  --wasm-code-gc (enable garbage collection of wasm code)
        type: bool  default: true
  --trace-wasm-code-gc (trace garbage collection of wasm code)
        type: bool  default: false
  --stress-wasm-code-gc (stress test garbage collection of wasm code)
        type: bool  default: false
  --wasm-max-initial-code-space-reservation (maximum size of the initial wasm code space reservation (in MB))
        type: int  default: 0
  --frame-count (number of stack frames inspected by the profiler)
        type: int  default: 1
  --stress-sampling-allocation-profiler (Enables sampling allocation profiler with X as a sample interval)
        type: int  default: 0
  --lazy-new-space-shrinking (Enables the lazy new space shrinking strategy)
        type: bool  default: false
  --min-semi-space-size (min size of a semi-space (in MBytes), the new space consists of two semi-spaces)
        type: size_t  default: 0
  --max-semi-space-size (max size of a semi-space (in MBytes), the new space consists of two semi-spaces)
        type: size_t  default: 0
  --semi-space-growth-factor (factor by which to grow the new space)
        type: int  default: 2
  --max-old-space-size (max size of the old space (in Mbytes))
        type: size_t  default: 0
  --max-heap-size (max size of the heap (in Mbytes) both max_semi_space_size and max_old_space_size take precedence. All three flags cannot be specified at the same time.)
        type: size_t  default: 0
  --initial-heap-size (initial size of the heap (in Mbytes))
        type: size_t  default: 0
  --huge-max-old-generation-size (Increase max size of the old space to 4 GB for x64 systems withthe physical memory bigger than 16 GB)
        type: bool  default: true
  --initial-old-space-size (initial old space size (in Mbytes))
        type: size_t  default: 0
  --global-gc-scheduling (enable GC scheduling based on global memory)
        type: bool  default: true
  --gc-global (always perform global GCs)
        type: bool  default: false
  --random-gc-interval (Collect garbage after random(0, X) allocations. It overrides gc_interval.)
        type: int  default: 0
  --gc-interval (garbage collect after <n> allocations)
        type: int  default: -1
  --retain-maps-for-n-gc (keeps maps alive for <n> old space garbage collections)
        type: int  default: 2
  --trace-gc (print one trace line following each garbage collection)
        type: bool  default: false
  --trace-gc-nvp (print one detailed trace line in name=value format after each garbage collection)
        type: bool  default: false
  --trace-gc-ignore-scavenger (do not print trace line after scavenger collection)
        type: bool  default: false
  --trace-idle-notification (print one trace line following each idle notification)
        type: bool  default: false
  --trace-idle-notification-verbose (prints the heap state used by the idle notification)
        type: bool  default: false
  --trace-gc-verbose (print more details following each garbage collection)
        type: bool  default: false
  --trace-gc-freelists (prints details of each freelist before and after each major garbage collection)
        type: bool  default: false
  --trace-gc-freelists-verbose (prints details of freelists of each page before and after each major garbage collection)
        type: bool  default: false
  --trace-evacuation-candidates (Show statistics about the pages evacuation by the compaction)
        type: bool  default: false
  --trace-allocations-origins (Show statistics about the origins of allocations. Combine with --no-inline-new to track allocations from generated code)
        type: bool  default: false
  --trace-allocation-stack-interval (print stack trace after <n> free-list allocations)
        type: int  default: -1
  --trace-duplicate-threshold-kb (print duplicate objects in the heap if their size is more than given threshold)
        type: int  default: 0
  --trace-fragmentation (report fragmentation for old space)
        type: bool  default: false
  --trace-fragmentation-verbose (report fragmentation for old space (detailed))
        type: bool  default: false
  --minor-mc-trace-fragmentation (trace fragmentation after marking)
        type: bool  default: false
  --trace-evacuation (report evacuation statistics)
        type: bool  default: false
  --trace-mutator-utilization (print mutator utilization, allocation speed, gc speed)
        type: bool  default: false
  --incremental-marking (use incremental marking)
        type: bool  default: true
  --incremental-marking-wrappers (use incremental marking for marking wrappers)
        type: bool  default: true
  --incremental-marking-task (use tasks for incremental marking)
        type: bool  default: true
  --incremental-marking-soft-trigger (threshold for starting incremental marking via a task in percent of available space: limit - size)
        type: int  default: 0
  --incremental-marking-hard-trigger (threshold for starting incremental marking immediately in percent of available space: limit - size)
        type: int  default: 0
  --trace-unmapper (Trace the unmapping)
        type: bool  default: false
  --parallel-scavenge (parallel scavenge)
        type: bool  default: true
  --scavenge-task (schedule scavenge tasks)
        type: bool  default: true
  --scavenge-task-trigger (scavenge task trigger in percent of the current heap limit)
        type: int  default: 80
  --scavenge-separate-stack-scanning (use a separate phase for stack scanning in scavenge)
        type: bool  default: false
  --trace-parallel-scavenge (trace parallel scavenge)
        type: bool  default: false
  --write-protect-code-memory (write protect code memory)
        type: bool  default: true
  --concurrent-marking (use concurrent marking)
        type: bool  default: true
  --concurrent-array-buffer-sweeping (concurrently sweep array buffers)
        type: bool  default: true
  --concurrent-allocation (concurrently allocate in old space)
        type: bool  default: false
  --local-heaps (allow heap access from background tasks)
        type: bool  default: false
  --stress-concurrent-allocation (start background threads that allocate memory)
        type: bool  default: false
  --parallel-marking (use parallel marking in atomic pause)
        type: bool  default: true
  --ephemeron-fixpoint-iterations (number of fixpoint iterations it takes to switch to linear ephemeron algorithm)
        type: int  default: 10
  --trace-concurrent-marking (trace concurrent marking)
        type: bool  default: false
  --concurrent-store-buffer (use concurrent store buffer processing)
        type: bool  default: true
  --concurrent-sweeping (use concurrent sweeping)
        type: bool  default: true
  --parallel-compaction (use parallel compaction)
        type: bool  default: true
  --parallel-pointer-update (use parallel pointer update during compaction)
        type: bool  default: true
  --detect-ineffective-gcs-near-heap-limit (trigger out-of-memory failure to avoid GC storm near heap limit)
        type: bool  default: true
  --trace-incremental-marking (trace progress of the incremental marking)
        type: bool  default: false
  --trace-stress-marking (trace stress marking progress)
        type: bool  default: false
  --trace-stress-scavenge (trace stress scavenge progress)
        type: bool  default: false
  --track-gc-object-stats (track object counts and memory usage)
        type: bool  default: false
  --trace-gc-object-stats (trace object counts and memory usage)
        type: bool  default: false
  --trace-zone-stats (trace zone memory usage)
        type: bool  default: false
  --zone-stats-tolerance (report a tick only when allocated zone memory changes by this amount)
        type: size_t  default: 1048576
  --track-retaining-path (enable support for tracking retaining path)
        type: bool  default: false
  --concurrent-array-buffer-freeing (free array buffer allocations on a background thread)
        type: bool  default: true
  --gc-stats (Used by tracing internally to enable gc statistics)
        type: int  default: 0
  --track-detached-contexts (track native contexts that are expected to be garbage collected)
        type: bool  default: true
  --trace-detached-contexts (trace native contexts that are expected to be garbage collected)
        type: bool  default: false
  --move-object-start (enable moving of object starts)
        type: bool  default: true
  --memory-reducer (use memory reducer)
        type: bool  default: true
  --memory-reducer-for-small-heaps (use memory reducer for small heaps)
        type: bool  default: true
  --heap-growing-percent (specifies heap growing factor as (1 + heap_growing_percent/100))
        type: int  default: 0
  --v8-os-page-size (override OS page size (in KBytes))
        type: int  default: 0
  --always-compact (Perform compaction on every full GC)
        type: bool  default: false
  --never-compact (Never perform compaction on full GC - testing only)
        type: bool  default: false
  --compact-code-space (Compact code space on full collections)
        type: bool  default: true
  --flush-bytecode (flush of bytecode when it has not been executed recently)
        type: bool  default: true
  --stress-flush-bytecode (stress bytecode flushing)
        type: bool  default: false
  --use-marking-progress-bar (Use a progress bar to scan large objects in increments when incremental marking is active.)
        type: bool  default: true
  --stress-per-context-marking-worklist (Use per-context worklist for marking)
        type: bool  default: false
  --force-marking-deque-overflows (force overflows of marking deque by reducing it's size to 64 words)
        type: bool  default: false
  --stress-compaction (stress the GC compactor to flush out bugs (implies --force_marking_deque_overflows))
        type: bool  default: false
  --stress-compaction-random (Stress GC compaction by selecting random percent of pages as evacuation candidates. It overrides stress_compaction.)
        type: bool  default: false
  --stress-incremental-marking (force incremental marking for small heaps and run it more often)
        type: bool  default: false
  --fuzzer-gc-analysis (prints number of allocations and enables analysis mode for gc fuzz testing, e.g. --stress-marking, --stress-scavenge)
        type: bool  default: false
  --stress-marking (force marking at random points between 0 and X (inclusive) percent of the regular marking start limit)
        type: int  default: 0
  --stress-scavenge (force scavenge at random points between 0 and X (inclusive) percent of the new space capacity)
        type: int  default: 0
  --gc-experiment-background-schedule (new background GC schedule heuristics)
        type: bool  default: false
  --gc-experiment-less-compaction (less compaction in non-memory reducing mode)
        type: bool  default: false
  --disable-abortjs (disables AbortJS runtime function)
        type: bool  default: false
  --randomize-all-allocations (randomize virtual memory reservations by ignoring any hints passed when allocating pages)
        type: bool  default: false
  --manual-evacuation-candidates-selection (Test mode only flag. It allows an unit test to select evacuation candidates pages (requires --stress_compaction).)
        type: bool  default: false
  --fast-promotion-new-space (fast promote new space on high survival rates)
        type: bool  default: false
  --clear-free-memory (initialize free memory with 0)
        type: bool  default: false
  --young-generation-large-objects (allocates large objects by default in the young generation large object space)
        type: bool  default: true
  --debug-code (generate extra code (assertions) for debugging)
        type: bool  default: false
  --code-comments (emit comments in code disassembly; for more readable source positions you should add --no-concurrent_recompilation)
        type: bool  default: false
  --enable-sse3 (enable use of SSE3 instructions if available)
        type: bool  default: true
  --enable-ssse3 (enable use of SSSE3 instructions if available)
        type: bool  default: true
  --enable-sse4-1 (enable use of SSE4.1 instructions if available)
        type: bool  default: true
  --enable-sse4-2 (enable use of SSE4.2 instructions if available)
        type: bool  default: true
  --enable-sahf (enable use of SAHF instruction if available (X64 only))
        type: bool  default: true
  --enable-avx (enable use of AVX instructions if available)
        type: bool  default: true
  --enable-fma3 (enable use of FMA3 instructions if available)
        type: bool  default: true
  --enable-bmi1 (enable use of BMI1 instructions if available)
        type: bool  default: true
  --enable-bmi2 (enable use of BMI2 instructions if available)
        type: bool  default: true
  --enable-lzcnt (enable use of LZCNT instruction if available)
        type: bool  default: true
  --enable-popcnt (enable use of POPCNT instruction if available)
        type: bool  default: true
  --arm-arch (generate instructions for the selected ARM architecture if available: armv6, armv7, armv7+sudiv or armv8)
        type: string  default: armv8
  --force-long-branches (force all emitted branches to be in long mode (MIPS/PPC only))
        type: bool  default: false
  --mcpu (enable optimization for specific cpu)
        type: string  default: auto
  --partial-constant-pool (enable use of partial constant pools (X64 only))
        type: bool  default: true
  --sim-arm64-optional-features (enable optional features on the simulator for testing: none or all)
        type: string  default: none
  --enable-source-at-csa-bind (Include source information in the binary at CSA bind locations.)
        type: bool  default: false
  --enable-armv7 (deprecated (use --arm_arch instead))
        type: maybe_bool  default: unset
  --enable-vfp3 (deprecated (use --arm_arch instead))
        type: maybe_bool  default: unset
  --enable-32dregs (deprecated (use --arm_arch instead))
        type: maybe_bool  default: unset
  --enable-neon (deprecated (use --arm_arch instead))
        type: maybe_bool  default: unset
  --enable-sudiv (deprecated (use --arm_arch instead))
        type: maybe_bool  default: unset
  --enable-armv8 (deprecated (use --arm_arch instead))
        type: maybe_bool  default: unset
  --enable-regexp-unaligned-accesses (enable unaligned accesses for the regexp engine)
        type: bool  default: true
  --script-streaming (enable parsing on background)
        type: bool  default: true
  --stress-background-compile (stress test parsing on background)
        type: bool  default: false
  --finalize-streaming-on-background (perform the script streaming finalization on the background thread)
        type: bool  default: false
  --disable-old-api-accessors (Disable old-style API accessors whose setters trigger through the prototype chain)
        type: bool  default: false
  --expose-gc (expose gc extension)
        type: bool  default: false
  --expose-gc-as (expose gc extension under the specified name)
        type: string  default: nullptr
  --expose-externalize-string (expose externalize string extension)
        type: bool  default: false
  --expose-trigger-failure (expose trigger-failure extension)
        type: bool  default: false
  --stack-trace-limit (number of stack frames to capture)
        type: int  default: 10
  --builtins-in-stack-traces (show built-in functions in stack traces)
        type: bool  default: false
  --experimental-stack-trace-frames (enable experimental frames (API/Builtins) and stack trace layout)
        type: bool  default: false
  --disallow-code-generation-from-strings (disallow eval and friends)
        type: bool  default: false
  --expose-async-hooks (expose async_hooks object)
        type: bool  default: false
  --expose-cputracemark-as (expose cputracemark extension under the specified name)
        type: string  default: nullptr
  --allow-unsafe-function-constructor (allow invoking the function constructor without security checks)
        type: bool  default: false
  --force-slow-path (always take the slow path for builtins)
        type: bool  default: false
  --test-small-max-function-context-stub-size (enable testing the function context size overflow path by making the maximum size smaller)
        type: bool  default: false
  --inline-new (use fast inline allocation)
        type: bool  default: true
  --trace (trace javascript function calls)
        type: bool  default: false
  --trace-wasm (trace wasm function calls)
        type: bool  default: false
  --lazy (use lazy compilation)
        type: bool  default: true
  --max-lazy (ignore eager compilation hints)
        type: bool  default: false
  --trace-opt (trace lazy optimization)
        type: bool  default: false
  --trace-opt-verbose (extra verbose compilation tracing)
        type: bool  default: false
  --trace-opt-stats (trace lazy optimization statistics)
        type: bool  default: false
  --trace-deopt (trace optimize function deoptimization)
        type: bool  default: false
  --trace-file-names (include file names in trace-opt/trace-deopt output)
        type: bool  default: false
  --always-opt (always try to optimize functions)
        type: bool  default: false
  --always-osr (always try to OSR functions)
        type: bool  default: false
  --prepare-always-opt (prepare for turning on always opt)
        type: bool  default: false
  --trace-serializer (print code serializer trace)
        type: bool  default: false
  --compilation-cache (enable compilation cache)
        type: bool  default: true
  --cache-prototype-transitions (cache prototype transitions)
        type: bool  default: true
  --parallel-compile-tasks (enable parallel compile tasks)
        type: bool  default: false
  --compiler-dispatcher (enable compiler dispatcher)
        type: bool  default: false
  --trace-compiler-dispatcher (trace compiler dispatcher activity)
        type: bool  default: false
  --cpu-profiler-sampling-interval (CPU profiler sampling interval in microseconds)
        type: int  default: 1000
  --trace-side-effect-free-debug-evaluate (print debug messages for side-effect-free debug-evaluate for testing)
        type: bool  default: false
  --hard-abort (abort by crashing)
        type: bool  default: true
  --expose-inspector-scripts (expose injected-script-source.js for debugging)
        type: bool  default: false
  --stack-size (default size of stack region v8 is allowed to use (in kBytes))
        type: int  default: 984
  --max-stack-trace-source-length (maximum length of function source code printed in a stack trace.)
        type: int  default: 300
  --clear-exceptions-on-js-entry (clear pending exceptions when entering JavaScript)
        type: bool  default: false
  --histogram-interval (time interval in ms for aggregating memory histograms)
        type: int  default: 600000
  --heap-profiler-trace-objects (Dump heap object allocations/movements/size_updates)
        type: bool  default: false
  --heap-profiler-use-embedder-graph (Use the new EmbedderGraph API to get embedder nodes)
        type: bool  default: true
  --heap-snapshot-string-limit (truncate strings to this length in the heap snapshot)
        type: int  default: 1024
  --sampling-heap-profiler-suppress-randomness (Use constant sample intervals to eliminate test flakiness)
        type: bool  default: false
  --use-idle-notification (Use idle notification to reduce memory footprint.)
        type: bool  default: true
  --trace-ic (trace inline cache state transitions for tools/ic-processor)
        type: bool  default: false
  --modify-field-representation-inplace (enable in-place field representation updates)
        type: bool  default: true
  --max-polymorphic-map-count (maximum number of maps to track in POLYMORPHIC state)
        type: int  default: 4
  --native-code-counters (generate extra code for manipulating stats counters)
        type: bool  default: false
  --thin-strings (Enable ThinString support)
        type: bool  default: true
  --trace-prototype-users (Trace updates to prototype user tracking)
        type: bool  default: false
  --trace-for-in-enumerate (Trace for-in enumerate slow-paths)
        type: bool  default: false
  --trace-maps (trace map creation)
        type: bool  default: false
  --trace-maps-details (also log map details)
        type: bool  default: true
  --allow-natives-syntax (allow natives syntax)
        type: bool  default: false
  --allow-natives-for-differential-fuzzing (allow only natives explicitly allowlisted for differential fuzzers)
        type: bool  default: false
  --parse-only (only parse the sources)
        type: bool  default: false
  --trace-sim (Trace simulator execution)
        type: bool  default: false
  --debug-sim (Enable debugging the simulator)
        type: bool  default: false
  --check-icache (Check icache flushes in ARM and MIPS simulator)
        type: bool  default: false
  --stop-sim-at (Simulator stop after x number of instructions)
        type: int  default: 0
  --sim-stack-alignment (Stack alingment in bytes in simulator (4 or 8, 8 is default))
        type: int  default: 8
  --sim-stack-size (Stack size of the ARM64, MIPS64 and PPC64 simulator in kBytes (default is 2 MB))
        type: int  default: 2048
  --log-colour (When logging, try to use coloured output.)
        type: bool  default: true
  --trace-sim-messages (Trace simulator debug messages. Implied by --trace-sim.)
        type: bool  default: false
  --async-stack-traces (include async stack traces in Error.stack)
        type: bool  default: true
  --stack-trace-on-illegal (print stack trace when an illegal exception is thrown)
        type: bool  default: false
  --abort-on-uncaught-exception (abort program (dump core) when an uncaught exception is thrown)
        type: bool  default: false
  --correctness-fuzzer-suppressions (Suppress certain unspecified behaviors to ease correctness fuzzing: Abort program when the stack overflows or a string exceeds maximum length (as opposed to throwing RangeError). Use a fixed suppression string for error messages.)
        type: bool  default: false
  --randomize-hashes (randomize hashes to avoid predictable hash collisions (with snapshots this option cannot override the baked-in seed))
        type: bool  default: true
  --rehash-snapshot (rehash strings from the snapshot to override the baked-in seed)
        type: bool  default: true
  --hash-seed (Fixed seed to use to hash property keys (0 means random)(with snapshots this option cannot override the baked-in seed))
        type: uint64  default: 0
  --random-seed (Default seed for initializing random generator (0, the default, means to use system random).)
        type: int  default: 0
  --fuzzer-random-seed (Default seed for initializing fuzzer random generator (0, the default, means to use v8's random number generator seed).)
        type: int  default: 0
  --trace-rail (trace RAIL mode)
        type: bool  default: false
  --print-all-exceptions (print exception object and stack trace on each thrown exception)
        type: bool  default: false
  --detailed-error-stack-trace (includes arguments for each function call in the error stack frames array)
        type: bool  default: false
  --adjust-os-scheduling-parameters (adjust OS specific scheduling params for the isolate)
        type: bool  default: true
  --runtime-call-stats (report runtime call counts and times)
        type: bool  default: false
  --rcs (report runtime call counts and times)
        type: bool  default: false
  --rcs-cpu-time (report runtime times in cpu time (the default is wall time))
        type: bool  default: false
  --profile-deserialization (Print the time it takes to deserialize the snapshot.)
        type: bool  default: false
  --serialization-statistics (Collect statistics on serialized objects.)
        type: bool  default: false
  --serialization-chunk-size (Custom size for serialization chunks)
        type: uint  default: 4096
  --regexp-optimization (generate optimized regexp code)
        type: bool  default: true
  --regexp-mode-modifiers (enable inline flags in regexp.)
        type: bool  default: false
  --regexp-interpret-all (interpret all regexp code)
        type: bool  default: false
  --regexp-tier-up (enable regexp interpreter and tier up to the compiler after the number of executions set by the tier up ticks flag)
        type: bool  default: true
  --regexp-tier-up-ticks (set the number of executions for the regexp interpreter before tiering-up to the compiler)
        type: int  default: 1
  --regexp-peephole-optimization (enable peephole optimization for regexp bytecode)
        type: bool  default: true
  --trace-regexp-peephole-optimization (trace regexp bytecode peephole optimization)
        type: bool  default: false
  --trace-regexp-bytecodes (trace regexp bytecode execution)
        type: bool  default: false
  --trace-regexp-assembler (trace regexp macro assembler calls.)
        type: bool  default: false
  --trace-regexp-parser (trace regexp parsing)
        type: bool  default: false
  --trace-regexp-tier-up (trace regexp tiering up execution)
        type: bool  default: false
  --testing-bool-flag (testing_bool_flag)
        type: bool  default: true
  --testing-maybe-bool-flag (testing_maybe_bool_flag)
        type: maybe_bool  default: unset
  --testing-int-flag (testing_int_flag)
        type: int  default: 13
  --testing-float-flag (float-flag)
        type: float  default: 2.5
  --testing-string-flag (string-flag)
        type: string  default: Hello, world!
  --testing-prng-seed (Seed used for threading test randomness)
        type: int  default: 42
  --testing-d8-test-runner (test runner turns on this flag to enable a check that the function was prepared for optimization before marking it for optimization)
        type: bool  default: false
  --fuzzing (Fuzzers use this flag to signal that they are ... fuzzing. This causes intrinsics to fail silently (e.g. return undefined) on invalid usage.)
        type: bool  default: false
  --embedded-src (Path for the generated embedded data file. (mksnapshot only))
        type: string  default: nullptr
  --embedded-variant (Label to disambiguate symbols in embedded data file. (mksnapshot only))
        type: string  default: nullptr
  --startup-src (Write V8 startup as C++ src. (mksnapshot only))
        type: string  default: nullptr
  --startup-blob (Write V8 startup blob file. (mksnapshot only))
        type: string  default: nullptr
  --target-arch (The mksnapshot target arch. (mksnapshot only))
        type: string  default: nullptr
  --target-os (The mksnapshot target os. (mksnapshot only))
        type: string  default: nullptr
  --target-is-simulator (Instruct mksnapshot that the target is meant to run in the simulator and it can generate simulator-specific instructions. (mksnapshot only))
        type: bool  default: false
  --minor-mc-parallel-marking (use parallel marking for the young generation)
        type: bool  default: true
  --trace-minor-mc-parallel-marking (trace parallel marking for the young generation)
        type: bool  default: false
  --minor-mc (perform young generation mark compact GCs)
        type: bool  default: false
  --help (Print usage message, including flags, on console)
        type: bool  default: true
  --dump-counters (Dump counters on exit)
        type: bool  default: false
  --dump-counters-nvp (Dump counters as name-value pairs on exit)
        type: bool  default: false
  --use-external-strings (Use external strings for source code)
        type: bool  default: false
  --map-counters (Map counters to a file)
        type: string  default:
  --mock-arraybuffer-allocator (Use a mock ArrayBuffer allocator for testing.)
        type: bool  default: false
  --mock-arraybuffer-allocator-limit (Memory limit for mock ArrayBuffer allocator used to simulate OOM for testing.)
        type: size_t  default: 0
  --gdbjit (enable GDBJIT interface)
        type: bool  default: false
  --gdbjit-full (enable GDBJIT interface for all code objects)
        type: bool  default: false
  --gdbjit-dump (dump elf objects with debug info to disk)
        type: bool  default: false
  --gdbjit-dump-filter (dump only objects containing this substring)
        type: string  default:
  --log (Minimal logging (no API, code, GC, suspect, or handles samples).)
        type: bool  default: false
  --log-all (Log all events to the log file.)
        type: bool  default: false
  --log-api (Log API events to the log file.)
        type: bool  default: false
  --log-code (Log code events to the log file without profiling.)
        type: bool  default: false
  --log-handles (Log global handle events.)
        type: bool  default: false
  --log-suspect (Log suspect operations.)
        type: bool  default: false
  --log-source-code (Log source code.)
        type: bool  default: false
  --log-function-events (Log function events (parse, compile, execute) separately.)
        type: bool  default: false
  --prof (Log statistical profiling information (implies --log-code).)
        type: bool  default: false
  --detailed-line-info (Always generate detailed line information for CPU profiling.)
        type: bool  default: false
  --prof-sampling-interval (Interval for --prof samples (in microseconds).)
        type: int  default: 1000
  --prof-cpp (Like --prof, but ignore generated code.)
        type: bool  default: false
  --prof-browser-mode (Used with --prof, turns on browser-compatible mode for profiling.)
        type: bool  default: true
  --logfile (Specify the name of the log file.)
        type: string  default: v8.log
  --logfile-per-isolate (Separate log files for each isolate.)
        type: bool  default: true
  --ll-prof (Enable low-level linux profiler.)
        type: bool  default: false
  --gc-fake-mmap (Specify the name of the file for fake gc mmap used in ll_prof)
        type: string  default: /tmp/__v8_gc__
  --log-internal-timer-events (Time internal events.)
        type: bool  default: false
  --redirect-code-traces (output deopt information and disassembly into file code-<pid>-<isolate id>.asm)
        type: bool  default: false
  --redirect-code-traces-to (output deopt information and disassembly into the given file)
        type: string  default: nullptr
  --print-opt-source (print source code of optimized and inlined functions)
        type: bool  default: false
  --vtune-prof-annotate-wasm (Used when v8_enable_vtunejit is enabled, load wasm source map and provide annotate support (experimental).)
        type: bool  default: false
  --win64-unwinding-info (Enable unwinding info for Windows/x64)
        type: bool  default: true
  --interpreted-frames-native-stack (Show interpreted frames on the native stack (useful for external profilers).)
        type: bool  default: false
  --predictable (enable predictable mode)
        type: bool  default: false
  --predictable-gc-schedule (Predictable garbage collection schedule. Fixes heap growing, idle, and memory reducing behavior.)
        type: bool  default: false
  --single-threaded (disable the use of background tasks)
        type: bool  default: false
  --single-threaded-gc (disable the use of background gc tasks)
        type: bool  default: false
```

Particularly useful ones:

```
--async-stack-trace
```

### Continuous Benchmarks

See our benchmarks [over here](https://deno.land/benchmarks)

The benchmark chart supposes
https://github.com/denoland/benchmark_data/blob/gh-pages/data.json has the type
`BenchmarkData[]` where `BenchmarkData` is defined like the below:

```ts
interface ExecTimeData {
  mean: number;
  stddev: number;
  user: number;
  system: number;
  min: number;
  max: number;
}

interface BenchmarkData {
  created_at: string;
  sha1: string;
  benchmark: {
    [key: string]: ExecTimeData;
  };
  binarySizeData: {
    [key: string]: number;
  };
  threadCountData: {
    [key: string]: number;
  };
  syscallCountData: {
    [key: string]: number;
  };
}
```
