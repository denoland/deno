# fetch — perf research

Macro-level performance research on Deno's implementation of `fetch` / `Request`
/ `Response` / `Headers` (client + server, body consumption, streaming bodies).

This directory contains only benchmark scripts, profile artifacts, and this
README. There are no production-code changes; the report lives in the PR body.

## Layout

```
servers/       cross-runtime HTTP servers (Deno.serve, node:http, Bun.serve)
clients/       cross-runtime fetch clients (rps drivers)
micro/         Headers / Request / Response microbenches (portable across runtimes)
profiles/      committed flamegraph excerpts and V8 prof output
run_servers.sh wrk-driven server bench (writes results.jsonl + versions.txt)
run_micro.sh   microbench runner (writes micro_results.jsonl)
```

## Runtime versions (this host)

See `versions.txt` after running `run_servers.sh`.

Pinned baselines used in the reports:

- Deno: built from this branch's `main` via `cargo build --release --bin deno`
- Node: `v22.22.2` (Node 22 LTS, fetch is undici-backed)
- Bun: `1.3.14`

## Reproduction

```bash
# from repo root
cargo build --release --bin deno

cd tools/perf_research/fetch
bash run_micro.sh                       # microbenches
bash run_servers.sh 10 64               # 10s @ 64 conns wrk runs
```

The harness expects `wrk`, `jq`, `node`, `bun`, and `./target/release/deno`
on PATH (or as `DENO_BIN`/`NODE_BIN`/`BUN_BIN` env vars).

## Caveats

This host is **Docker inside a Proxmox VM**, so absolute throughput numbers are
unreliable. The report leads with same-host ratios vs. Node + Bun and with
flamegraph attribution, not absolute rps.

`perf` and `samply` require `kernel.perf_event_paranoid<=1` — the container is
locked at `3` and `sysctl` is denied even via `sudo`. Profile attribution in
this report therefore comes from V8's in-process `--prof` (always works).
