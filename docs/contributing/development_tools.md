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
deno run -A --unstable ./tools/lint.js
```

Format the code:

```shell
deno run -A --unstable ./tools/format.js
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
