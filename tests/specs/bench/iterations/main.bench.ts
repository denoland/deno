Deno.bench("above 10,000,000 iterations", {
  iterations: 10_000_001,
  warmups: 10,
}, () => {
});

Deno.bench("below 10,000,000 iterations", {
  iterations: 1,
  warmups: 10,
}, () => {
});

Deno.bench("negative iterations", {
  iterations: -10,
  warmups: -10,
}, () => {
});
