Deno.bench("bench0", () => {});
Deno.bench(function bench1() {});
Deno.bench({ name: "bench2", fn: () => {} });
Deno.bench("bench3", { permissions: "none" }, () => {});
Deno.bench({ name: "bench4" }, () => {});
Deno.bench({ ignore: true }, function bench5() {});
