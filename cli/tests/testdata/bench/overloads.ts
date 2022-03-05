Deno.bench("test0", () => {});
Deno.bench(function test1() {});
Deno.bench({ name: "test2", fn: () => {} });
Deno.bench("test3", { permissions: "none" }, () => {});
Deno.bench({ name: "test4" }, () => {});
Deno.bench({ ignore: true }, function test5() {});
