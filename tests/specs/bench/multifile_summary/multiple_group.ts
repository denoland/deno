Deno.bench("noop", { group: "noop" }, () => {});
Deno.bench("noop2", { group: "noop", baseline: true }, () => {});

Deno.bench("noop3", { group: "url" }, () => {});

Deno.bench("parse url 2x", { group: "url", baseline: true }, () => {
  new URL("https://jsr.io/@std/http/0.221.0/file_server.ts");
  new URL("https://jsr.io/@std/http/0.221.0/file_server.ts");
});

Deno.bench("parse url 200x", { group: "url" }, () => {
  for (let i = 0; i < 200; i++) {
    new URL("https://jsr.io/@std/http/0.221.0/file_server.ts");
  }
});
