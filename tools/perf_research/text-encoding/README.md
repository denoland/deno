# text-encoding — perf research

Macro-level performance research on Deno's implementation of `TextEncoder` and
`TextDecoder` (including `encodeInto` and stream-mode decoding).

Bench scripts and committed V8 prof artifacts only; the PR body is the report.

## Layout

```
micro/text_encoding_micro.js   14-op portable microbench
profiles/                      V8 prof output (.prof.txt + .v8.log.gz)
```

## Pinned versions

- Deno: built from this branch's `main` via `cargo build --release --bin deno` (2.7.14, v8 14.7.173.20-rusty)
- Node: `v22.22.2`
- Bun: `1.3.14`

## Reproduction

```bash
cargo build --release --bin deno
for rt in deno node bun; do
  case $rt in
    deno) ./target/release/deno run -A --no-prompt tools/perf_research/text-encoding/micro/text_encoding_micro.js ;;
    node) node tools/perf_research/text-encoding/micro/text_encoding_micro.js ;;
    bun)  bun  tools/perf_research/text-encoding/micro/text_encoding_micro.js ;;
  esac
done

# V8 prof (perf/samply blocked by container caps)
mkdir -p /tmp/textprof && cd /tmp/textprof
$DENO_BIN run -A --no-prompt --v8-flags=--prof,--no-logfile-per-isolate \
    /path/to/tools/perf_research/text-encoding/micro/text_encoding_micro.js
node --prof-process v8.log > /path/to/profiles/text_encoding_micro.prof.txt
```

## Caveats

Docker on Proxmox VM → absolute numbers unreliable; report leads with ratios
and V8 `--prof` attribution. `perf` / `samply` blocked (`paranoid=3`); V8
`--prof` is in-process and always works.
