Deno.bench("above 10,000,000 iterations", {
  n: 10_000_001,
  warmup: 10,
}, () => {
});

Deno.bench("below 10,000,000 iterations", {
  n: 1,
  warmup: 10,
}, () => {
});

Deno.bench("negative iterations", {
  n: -10,
  warmup: -10,
}, () => {
});
