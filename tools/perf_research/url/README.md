# url — perf research

Macro-level performance research on Deno's implementation of `URL` and
`URLSearchParams`.

Bench scripts and committed V8 prof artifacts only; the PR body is the report.

## Layout

```
micro/url_micro.js   URL + URLSearchParams microbench (portable across runtimes)
profiles/            committed V8 prof output (.prof.txt + .v8.log.gz)
```

## Pinned versions (this host)

- Deno: built from this branch's `main` via `cargo build --release --bin deno` (deno 2.7.14, v8 14.7.173.20-rusty)
- Node: `v22.22.2`
- Bun: `1.3.14`

## Reproduction

```bash
cargo build --release --bin deno

# microbenches
./target/release/deno run -A --no-prompt tools/perf_research/url/micro/url_micro.js
node tools/perf_research/url/micro/url_micro.js
bun  tools/perf_research/url/micro/url_micro.js

# V8 prof (in-process; perf_event_paranoid=3 in this container blocks `perf`/`samply`)
mkdir -p /tmp/urlprof && cd /tmp/urlprof
$DENO_BIN run -A --no-prompt --v8-flags=--prof,--no-logfile-per-isolate \
    /path/to/tools/perf_research/url/micro/url_micro.js
node --prof-process v8.log > /path/to/profiles/url_micro.prof.txt
```

## Caveats

This host is **Docker inside a Proxmox VM**, so absolute throughput numbers are
unreliable. The report leads with same-host ratios vs Node + Bun and with V8
`--prof` attribution.
