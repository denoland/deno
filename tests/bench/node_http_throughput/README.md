# node:http throughput benchmark

Manual throughput harness for comparing two release-lite Deno binaries on the
same execution target. This is intentionally not wired into CI because results
depend on CPU pinning, host load, and run-order drift.

Example control-vs-candidate run:

```sh
./target/release-lite/deno run \
  --allow-read --allow-run --allow-write --allow-env \
  tests/bench/node_http_throughput/run.ts \
  --target-id bigboi \
  --deno ./target/release-lite/deno \
  --candidate-deno /path/to/candidate/deno \
  --duration 30 \
  --samples 15 \
  --warmups 2 \
  --connections 128 \
  --out-dir target/node_http_throughput
```

By default the harness starts one server for the control binary and one server
for the candidate binary, warms both, and then alternates measured `wrk` samples
between them. Use `--order random --seed <n>` to use a reproducible randomized
sample order instead.

The JSON and Markdown outputs record execution target id, git HEAD, binary
paths, full command line, CPU pinning, sample order, per-sample `wrk` output,
summary statistics, and the candidate-vs-control Welch confidence interval.
