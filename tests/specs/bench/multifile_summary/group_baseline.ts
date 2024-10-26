Deno.bench("noop", () => {});
Deno.bench("noop2", { baseline: true }, () => {});

Deno.bench("noop3", { group: "url" }, () => {});

Deno.bench("parse url 2x", { group: "url", baseline: true }, () => {
  new URL("https://deno.land/std/http/server.ts");
  new URL("https://deno.land/std/http/server.ts");
});

Deno.bench("parse url 200x", { group: "url" }, () => {
  for (let i = 0; i < 200; i++) {
    new URL("https://deno.land/std/http/server.ts");
  }
});
