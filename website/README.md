## About benchmark data

The benchmark chart supposes `//website/data.json` has the signature of `BenchmarkData[]` where `BenchmarkData` is defined like the below:

```typescript
interface ExecTimeData {
  mean: number
  stddev: number
  user: number
  system: number
  min: number
  max: number
}

interface BenchmarkData {
  created_at: string,
  sha1: string,
  binary_size?: number,
  benchmark: {
    [key: string]: ExecTimeData
  }
}
```
