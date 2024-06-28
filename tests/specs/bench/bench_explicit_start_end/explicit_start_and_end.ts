Deno.bench("start and end", (t) => {
  const id = setInterval(() => {}, 1000);
  t.start();
  Deno.inspect(id);
  t.end();
  clearInterval(id);
});

Deno.bench("start only", (t) => {
  const id = setInterval(() => {}, 1000);
  t.start();
  Deno.inspect(id);
  clearInterval(id);
});

Deno.bench("end only", (t) => {
  const id = setInterval(() => {}, 1000);
  Deno.inspect(id);
  t.end();
  clearInterval(id);
});

Deno.bench("double start", (t) => {
  const id = setInterval(() => {}, 1000);
  t.start();
  t.start();
  Deno.inspect(id);
  t.end();
  clearInterval(id);
});

let captured: Deno.BenchContext;

Deno.bench("double end", (t) => {
  captured = t;
  const id = setInterval(() => {}, 1000);
  t.start();
  Deno.inspect(id);
  t.end();
  t.end();
  clearInterval(id);
});

Deno.bench("captured", () => {
  const id = setInterval(() => {}, 1000);
  captured!.start();
  Deno.inspect(id);
  captured!.end();
  clearInterval(id);
});
