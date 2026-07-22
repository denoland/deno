clearTimeout(setTimeout(() => {}, 1000));

Deno.bench("bench1", () => {});
Deno.bench("bench2", () => {});
Deno.bench("bench3", () => {});
