Deno.bench("URL parsing", { n: 250 }, () => {
  new URL("https://deno.land");
});

Deno.bench("URL resolving", { n: 1000 }, () => {
  new URL("./foo.js", import.meta.url);
});
