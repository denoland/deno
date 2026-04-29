```bash
% target/main-f092a500d6/release/deno bench matrix.ts
    CPU | Apple M4
Runtime | Deno 2.7.14 (aarch64-apple-darwin)

| benchmark              | time/iter (avg) |        iter/s |      (min … max)      |      p75 |      p99 |     p995 |
| ---------------------- | --------------- | ------------- | --------------------- | -------- | -------- | -------- |
| 2d                     |        101.1 ns |     9,890,000 | ( 90.9 ns … 285.1 ns) | 104.5 ns | 127.8 ns | 236.9 ns |
| 2d-readonly            |         99.6 ns |    10,040,000 | ( 90.3 ns … 300.3 ns) |  99.0 ns | 154.6 ns | 202.6 ns |
| 2d-sequence            |        875.4 ns |     1,142,000 | (863.1 ns … 889.5 ns) | 880.1 ns | 889.5 ns | 889.5 ns |
| 2d-sequence-readonly   |        884.1 ns |     1,131,000 | (862.2 ns … 910.4 ns) | 889.9 ns | 910.4 ns | 910.4 ns |
| 3d                     |         97.0 ns |    10,310,000 | ( 86.0 ns … 302.9 ns) |  97.6 ns | 161.4 ns | 202.2 ns |
| 3d-readonly            |        107.1 ns |     9,334,000 | ( 94.4 ns … 254.2 ns) | 106.6 ns | 194.7 ns | 234.5 ns |
| 3d-sequence            |          2.0 µs |       498,600 | (  1.9 µs …   3.4 µs) |   2.0 µs |   3.4 µs |   3.4 µs |
| 3d-sequence-readonly   |          1.9 µs |       540,300 | (  1.8 µs …   1.9 µs) |   1.9 µs |   1.9 µs |   1.9 µs |

% target/main-f092a500d6/release/deno bench matrix.ts
    CPU | Apple M4
Runtime | Deno 2.7.14 (aarch64-apple-darwin)

| benchmark              | time/iter (avg) |        iter/s |      (min … max)      |      p75 |      p99 |     p995 |
| ---------------------- | --------------- | ------------- | --------------------- | -------- | -------- | -------- |
| 2d                     |        114.1 ns |     8,768,000 | ( 89.8 ns … 314.1 ns) | 113.4 ns | 250.0 ns | 266.8 ns |
| 2d-readonly            |         92.9 ns |    10,760,000 | ( 83.2 ns … 343.7 ns) |  92.6 ns | 151.4 ns | 172.2 ns |
| 2d-sequence            |        889.5 ns |     1,124,000 | (863.7 ns … 979.2 ns) | 892.5 ns | 979.2 ns | 979.2 ns |
| 2d-sequence-readonly   |        874.9 ns |     1,143,000 | (863.6 ns … 888.9 ns) | 877.6 ns | 888.9 ns | 888.9 ns |
| 3d                     |         97.1 ns |    10,290,000 | ( 82.6 ns … 213.9 ns) |  98.1 ns | 151.2 ns | 175.9 ns |
| 3d-readonly            |         97.6 ns |    10,250,000 | ( 89.4 ns … 322.7 ns) |  97.5 ns | 136.9 ns | 167.1 ns |
| 3d-sequence            |          1.9 µs |       533,200 | (  1.8 µs …   2.0 µs) |   1.9 µs |   2.0 µs |   2.0 µs |
| 3d-sequence-readonly   |          1.9 µs |       521,800 | (  1.9 µs …   2.3 µs) |   1.9 µs |   2.3 µs |   2.3 µs |
```

```bash
% target/perf-dom-matrix-a38e06b817/release/deno bench matrix.ts
    CPU | Apple M4
Runtime | Deno 2.7.14 (aarch64-apple-darwin)

| benchmark              | time/iter (avg) |        iter/s |      (min … max)      |      p75 |      p99 |     p995 |
| ---------------------- | --------------- | ------------- | --------------------- | -------- | -------- | -------- |
| 2d                     |         91.8 ns |    10,890,000 | ( 77.5 ns … 376.4 ns) |  88.2 ns | 241.3 ns | 262.0 ns |
| 2d-readonly            |         82.3 ns |    12,150,000 | ( 69.9 ns … 379.3 ns) |  80.1 ns | 207.4 ns | 232.1 ns |
| 2d-sequence            |        848.9 ns |     1,178,000 | (821.5 ns …   1.3 µs) | 844.4 ns |   1.3 µs |   1.3 µs |
| 2d-sequence-readonly   |        828.4 ns |     1,207,000 | (810.6 ns …   1.2 µs) | 824.1 ns |   1.2 µs |   1.2 µs |
| 3d                     |         73.3 ns |    13,640,000 | ( 65.4 ns … 372.3 ns) |  73.1 ns | 110.0 ns | 133.2 ns |
| 3d-readonly            |         78.4 ns |    12,750,000 | ( 68.5 ns … 295.3 ns) |  78.1 ns | 131.8 ns | 175.4 ns |
| 3d-sequence            |          1.8 µs |       563,200 | (  1.8 µs …   1.8 µs) |   1.8 µs |   1.8 µs |   1.8 µs |
| 3d-sequence-readonly   |          1.8 µs |       541,300 | (  1.8 µs …   3.5 µs) |   1.8 µs |   3.5 µs |   3.5 µs |

% target/perf-dom-matrix-a38e06b817/release/deno bench matrix.ts
    CPU | Apple M4
Runtime | Deno 2.7.14 (aarch64-apple-darwin)

| benchmark              | time/iter (avg) |        iter/s |      (min … max)      |      p75 |      p99 |     p995 |
| ---------------------- | --------------- | ------------- | --------------------- | -------- | -------- | -------- |
| 2d                     |         93.2 ns |    10,730,000 | ( 70.6 ns … 553.1 ns) |  87.0 ns | 281.1 ns | 400.1 ns |
| 2d-readonly            |         74.7 ns |    13,390,000 | ( 66.7 ns … 298.9 ns) |  74.3 ns | 112.3 ns | 144.0 ns |
| 2d-sequence            |        852.5 ns |     1,173,000 | (825.3 ns …   1.1 µs) | 872.1 ns |   1.1 µs |   1.1 µs |
| 2d-sequence-readonly   |        828.8 ns |     1,207,000 | (814.2 ns … 952.2 ns) | 830.7 ns | 952.2 ns | 952.2 ns |
| 3d                     |         72.0 ns |    13,890,000 | ( 62.8 ns … 402.6 ns) |  71.8 ns | 113.2 ns | 126.0 ns |
| 3d-readonly            |         75.0 ns |    13,330,000 | ( 67.2 ns … 251.5 ns) |  74.5 ns | 122.8 ns | 125.0 ns |
| 3d-sequence            |          1.8 µs |       555,900 | (  1.8 µs …   1.9 µs) |   1.9 µs |   1.9 µs |   1.9 µs |
| 3d-sequence-readonly   |          1.8 µs |       556,500 | (  1.8 µs …   2.1 µs) |   1.8 µs |   2.1 µs |   2.1 µs |
```

## JSON comparison

Based on:

- `matrix_main.json`:
  `target/main-f092a500d6/release/deno bench matrix.ts --json`
- `matrix_this_pr.json`:
  `target/perf-dom-matrix-a38e06b817/release/deno bench matrix.ts --json`

Lower is better.

| benchmark            | avg (main → PR)    | avg Δ  | avg speedup | p75 (main → PR)    | p75 Δ  | p75 speedup | p99 (main → PR)    | p99 Δ  | p99 speedup |
| -------------------- | ------------------ | ------ | ----------- | ------------------ | ------ | ----------- | ------------------ | ------ | ----------- |
| 2d                   | 110.6 ns → 81.4 ns | -26.5% | 1.36x       | 112.8 ns → 82.9 ns | -26.6% | 1.36x       | 203.9 ns → 123.1 ns | -39.6% | 1.66x       |
| 2d-readonly          | 99.4 ns → 76.5 ns  | -23.0% | 1.30x       | 97.0 ns → 75.8 ns  | -21.9% | 1.28x       | 199.3 ns → 131.6 ns | -34.0% | 1.51x       |
| 2d-sequence          | 876.8 ns → 836.3 ns | -4.6% | 1.05x      | 878.6 ns → 838.6 ns | -4.6% | 1.05x      | 940.2 ns → 927.9 ns | -1.3% | 1.01x       |
| 2d-sequence-readonly | 890.4 ns → 829.0 ns | -6.9% | 1.07x      | 904.3 ns → 833.2 ns | -7.9% | 1.09x      | 993.0 ns → 890.8 ns | -10.3% | 1.11x       |
| 3d                   | 95.5 ns → 73.1 ns  | -23.5% | 1.31x       | 97.1 ns → 72.8 ns  | -25.0% | 1.33x       | 145.1 ns → 126.0 ns | -13.2% | 1.15x       |
| 3d-readonly          | 105.3 ns → 76.4 ns | -27.5% | 1.38x       | 105.5 ns → 76.1 ns | -27.9% | 1.39x       | 142.5 ns → 117.8 ns | -17.3% | 1.21x       |
| 3d-sequence          | 1.89 µs → 1.78 µs  | -6.0%  | 1.06x       | 1.87 µs → 1.78 µs  | -4.8%  | 1.05x       | 2.59 µs → 1.83 µs   | -29.3% | 1.42x       |
| 3d-sequence-readonly | 1.86 µs → 1.95 µs  | +4.5%  | 0.96x       | 1.87 µs → 2.01 µs  | +7.2%  | 0.93x       | 2.07 µs → 2.88 µs   | +38.9% | 0.72x       |

Observations:

- `fromFloat32Array()` paths still improve most, roughly `23%` to `28%` on
  `avg`.
- sequence constructor paths mostly improve, roughly `5%` to `7%` on `avg`,
  but `3d-sequence-readonly` regresses in this JSON pair.
- The largest steady win in this JSON pair is `3d-readonly` at `avg`
  (`-27.5%`) and `p75` (`-27.9%`).
- The largest single percentile win is `2d` at `p99` (`1.66x`).
- The two text runs above still show `3d-sequence-readonly` at about `1.8 µs`
  vs `1.9 µs` on `main`, so the JSON regression there looks tail-sensitive.
