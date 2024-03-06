Deno.bench("noop with start and end", (b) => {
  b.start();
  b.end();
});
