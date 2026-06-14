# Default test-runner output: a side-by-side comparison

A comparison of the **default** console output of the major JavaScript/TypeScript
test runners. The goal is to see what a developer actually sees out of the box —
no custom reporters, no config — for two scenarios:

1. **All tests pass** (with one skipped test)
2. **One test fails**

## Methodology

The exact same logical test suite was used everywhere: two `describe` groups
(`math`, `strings`), four tests total, one of which is skipped, and — in the
"fail" variant — one assertion changed from `2 * 3 === 6` to `2 * 3 === 7`.

| Runner | Version | Command |
| --- | --- | --- |
| Deno | 2.8.3 | `deno test file_test.ts` |
| Vitest | 4.1.8 | `vitest run file.test.js` |
| Bun | 1.3.11 | `bun test file.test.js` |
| Node | 22.22.2 | `node --test file.test.mjs` |
| Jest | latest (30.x) | `jest file.test.js` |

All runs were captured with `NO_COLOR=1` and output piped to a file (non-TTY),
so the listings below are plain text. **Color and TTY behavior matters a lot**
(see [Caveats](#caveats-tty-color-and-ci)) — in a real terminal most of these
add color, spinners, and per-file status lines.

---

## 1. Passing run

### Deno
```
Check deno/math_test.ts
running 4 tests from ./deno/math_test.ts
math - adds numbers ... ok (17ms)
math - multiplies numbers ... ok (0ms)
strings - concatenates ... ok (0ms)
strings - uppercases (todo) ... ignored (0ms)

ok | 3 passed | 0 failed | 1 ignored (52ms)
```
One line per test with a per-test duration, grouped by source file, plus a
single summary line. Type-checks the file first (`Check ...`).

### Vitest
```
 RUN  v4.1.8 /tmp/cmp

 Test Files  1 passed (1)
      Tests  3 passed | 1 skipped (4)
   Start at  18:05:22
   Duration  232ms (transform 22ms, setup 0ms, import 36ms, tests 4ms, environment 0ms)
```
Aggregated counts only — **individual passing tests are not listed by default**.
Note the detailed timing breakdown (transform / setup / import / tests /
environment).

### Bun
```
bun test v1.3.11 (af24e281)

 3 pass
 1 skip
 0 fail
 3 expect() calls
Ran 4 tests across 1 file. [95.00ms]
```
Very terse. Passing tests are not listed; it also reports the number of
`expect()` calls — a nudge against tests that assert nothing.

### Node (`node --test`)
Node's **default reporter depends on the TTY**: `spec` when stdout is a
terminal, `tap` when piped. Both shown.

`spec` (terminal):
```
▶ math
  ✔ adds numbers (0.702959ms)
  ✔ multiplies numbers (0.154437ms)
✔ math (1.876451ms)
▶ strings
  ✔ concatenates (0.181249ms)
  ﹣ uppercases (todo) (2.085659ms) # SKIP
✔ strings (2.499857ms)
ℹ tests 4
ℹ suites 2
ℹ pass 3
ℹ fail 0
ℹ cancelled 0
ℹ skipped 1
ℹ todo 0
ℹ duration_ms 110.085366
```

`tap` (piped / CI — this is the default when output is redirected):
```
TAP version 13
# Subtest: math
    # Subtest: adds numbers
    ok 1 - adds numbers
      ---
      duration_ms: 0.770886
      type: 'test'
      ...
    # Subtest: multiplies numbers
    ok 2 - multiplies numbers
      ...
    1..2
ok 1 - math
  ...
1..2
# tests 4
# suites 2
# pass 3
# fail 0
# cancelled 0
# skipped 1
# todo 0
# duration_ms 126.125492
```
TAP is machine-readable but extremely verbose for humans — note this is what you
get by default in a pipe/CI unless you pass `--test-reporter=spec`.

### Jest
```
Test Suites: 1 passed, 1 total
Tests:       1 skipped, 3 passed, 4 total
Snapshots:   0 total
Time:        0.235 s, estimated 1 s
Ran all test suites matching jest/math.test.js.
```
Summary block only (no per-test lines unless `--verbose`). In an actual terminal
Jest also prints a colored `PASS jest/math.test.js` line per file; when piped it
omits the green `PASS` lines (but keeps `FAIL` lines — see below). It tracks
snapshots as a first-class concept.

---

## 2. Failing run

This is where the runners differ the most, and where default output quality
really counts.

### Deno
```
running 4 tests from ./deno/mathfail_test.ts
math - adds numbers ... ok (18ms)
math - multiplies numbers ... FAILED (0ms)
strings - concatenates ... ok (0ms)
strings - uppercases (todo) ... ignored (0ms)

 ERRORS 

math - multiplies numbers => ./deno/mathfail_test.ts:6:6
error: AssertionError: Expected values to be strictly equal:

6 !== 7

  assert.strictEqual(2 * 3, 7);
         ^
    at file:///tmp/cmp/deno/mathfail_test.ts:7:10

 FAILURES 

math - multiplies numbers => ./deno/mathfail_test.ts:6:6

FAILED | 2 passed | 1 failed | 1 ignored (56ms)

error: Test failed
```
Failures are collected into an `ERRORS` section (full message + source frame +
stack) and then re-listed compactly in a `FAILURES` section with a
clickable `file:line:col` — handy when scrolling back through a long run.

### Vitest
```
 ❯ vitest/mathfail.test.js (4 tests | 1 failed | 1 skipped) 10ms
     × multiplies numbers 7ms

⎯⎯⎯⎯⎯⎯⎯ Failed Tests 1 ⎯⎯⎯⎯⎯⎯⎯

 FAIL  vitest/mathfail.test.js > math > multiplies numbers
AssertionError: expected 6 to be 7 // Object.is equality

- Expected
+ Received

- 7
+ 6

 ❯ vitest/mathfail.test.js:8:19
      6|   });
      7|   it("multiplies numbers", () => {
      8|     expect(2 * 3).toBe(7);
       |                   ^
      9|   });
     10| });

 Test Files  1 failed (1)
      Tests  1 failed | 2 passed | 1 skipped (4)
```
The richest failure output: full breadcrumb path (`file > describe > test`), a
colored **expected/received diff**, and a code frame with the offending line
underlined.

### Bun
```
bun/mathfail.test.js:
5 |   it("multiplies numbers", () => {
6 |     expect(2 * 3).toBe(7);
                      ^
error: expect(received).toBe(expected)

Expected: 7
Received: 6

      at <anonymous> (/tmp/cmp/bun/mathfail.test.js:6:19)
(fail) math > multiplies numbers [1.56ms]

 2 pass
 1 skip
 1 fail
 3 expect() calls
Ran 4 tests across 1 file. [36.00ms]
```
Shows a source frame at the failure point, expected/received (no diff), the
breadcrumb on the `(fail)` line, and the same compact summary.

### Node (`spec` reporter)
```
▶ math
  ✔ adds numbers (0.742974ms)
  ✖ multiplies numbers (0.925906ms)
✖ math (2.848943ms)
...
✖ failing tests:

test at node/mathfail.test.mjs:8:3
✖ multiplies numbers (0.925906ms)
  AssertionError [ERR_ASSERTION]: Expected values to be strictly equal:

  6 !== 7

      at TestContext.<anonymous> (file:///tmp/cmp/node/mathfail.test.mjs:9:12)
      at Test.runInAsyncScope (node:async_hooks:214:14)
      at Test.run (node:internal/test_runner/test:1047:25)
      ... (full internal stack)
    generatedMessage: true,
    code: 'ERR_ASSERTION',
    actual: 6,
    expected: 7,
    operator: 'strictEqual',
    diff: 'simple'
  }
```
Failing tests are re-collected at the bottom. The error dump includes the **full
internal Node stack frames** (`node:internal/test_runner/...`), which is noisy —
no code frame, no clean diff. (The `tap` reporter embeds the same info as YAML
inside the TAP stream.)

### Jest
```
FAIL jest/mathfail.test.js
  ● math › multiplies numbers

    expect(received).toBe(expected) // Object.is equality

    Expected: 7
    Received: 6

      4 |   });
      5 |   it("multiplies numbers", () => {
    > 6 |     expect(2 * 3).toBe(7);
        |                   ^
      7 |   });
      8 | });
      9 |

      at Object.toBe (jest/mathfail.test.js:6:19)

Test Suites: 1 failed, 1 total
Tests:       1 failed, 1 skipped, 2 passed, 4 total
```
Clean, well-established format: `describe › test` breadcrumb, expected/received,
and a code frame with the line marked by `>` and the column by `^`. No internal
framework frames in the user-facing stack.

---

## Summary

| Dimension | Deno | Vitest | Bun | Node `--test` | Jest |
| --- | --- | --- | --- | --- | --- |
| Lists each passing test by default | ✅ yes | ❌ counts only | ❌ counts only | ✅ (spec) / ✅ (tap) | ❌ counts only |
| Per-test durations | ✅ | only failed | only failed | ✅ | with `--verbose` |
| Expected/received **diff** | text (`6 !== 7`) | ✅ colored diff | text | text (`6 !== 7`) | text |
| Code frame at failure | ✅ (1 line) | ✅ multi-line | ✅ multi-line | ❌ | ✅ multi-line |
| Failure breadcrumb (`suite > test`) | flat name | ✅ | ✅ | tree | ✅ (`›`) |
| Re-collects failures at end | ✅ | ✅ | ❌ inline | ✅ (spec) | inline per file |
| Leaks internal stack frames | ❌ | ❌ | ❌ | ⚠️ yes | ❌ |
| Reports `expect()` call count | ❌ | ❌ | ✅ | ❌ | ❌ |
| Default reporter is machine-readable | ❌ | ❌ | ❌ | ✅ TAP (when piped) | ❌ |
| Type-checks TS before running | ✅ | ❌ (transpiles) | ❌ (transpiles) | n/a | ❌ |
| Needs config to run at all | ❌ | ❌ | ❌ | ❌ | ⚠️ often (TS/ESM) |
| Test API available as globals | n/a (`Deno.test`) | ❌ must import | ✅ | ❌ must import | ✅ |

### Quick take

- **Vitest** has the most informative default failure output (colored diff +
  multi-line code frame + breadcrumb), at the cost of being the quietest on
  success.
- **Jest** is the familiar baseline: clean failures, summary-only on success.
  Vitest deliberately mirrors it.
- **Deno** is the only one that lists every test *and* gives a tidy two-part
  (`ERRORS` / `FAILURES`) failure report with clickable locations, and the only
  one that type-checks first. Output is the same whether piped or not.
- **Bun** is the most terse and the fastest to read for a green run; failure
  output is good but doesn't aggregate. The `expect() calls` counter is a nice
  touch.
- **Node `--test`** is the odd one out: its default switches between human
  (`spec`) and machine (`tap`) output based on the TTY, and failure stacks
  include framework internals. Zero-dependency, but the rawest UX.

## Caveats: TTY, color, and CI

The listings above were captured **non-interactively** (piped, `NO_COLOR=1`),
which is what CI logs look like. In an interactive terminal the experience
differs:

- **Vitest, Jest, Bun** add color, and Vitest/Jest render a live-updating
  progress UI. Jest prints a colored `PASS`/`FAIL` line per file in a TTY but
  omits the green `PASS` lines when piped.
- **Node** silently switches its default reporter: `spec` (human) in a TTY,
  `tap` (machine) when redirected. Force one with `--test-reporter=spec|tap|dot`.
- **Deno** produces identical text in both modes (only color is added in a TTY),
  which makes its CI logs and local runs match.
- Jest and Vitest also default to **watch-mode-adjacent behavior** in some
  setups; the runs here used `jest` (single run) and `vitest run` (the
  non-watch command — plain `vitest` watches by default).

## Other runners worth knowing

- **Mocha** — the classic; pluggable reporters (`spec` default), needs a
  separate assertion lib (Chai).
- **AVA** — minimal, parallel-by-default, concise TAP-ish output.
- **uvu / tape / node-tap** — tiny, TAP-oriented runners.
- **Playwright Test / Cypress** — primarily for browser/e2e but ship their own
  runners and reporters.
- Note Deno can also run `node:test`-style and BDD (`describe`/`it`) suites via
  `jsr:@std/testing/bdd`, and Bun/Vitest are largely Jest-API compatible — so
  the *API* is often portable even when the default *output* is not.
