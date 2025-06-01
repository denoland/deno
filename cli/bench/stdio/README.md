## `stdio` benchmarks

Compile the C baseline and run the benchmark:

```bash
cc stdio.c -o stdio -O3
time dd if=/dev/zero bs=65536 count=500000 | ./stdio
time dd if=/dev/zero bs=65536 count=500000 | deno run stdio.js
```
