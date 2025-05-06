Deno.test("test0", () => {});
Deno.test(function test1() {});
Deno.test({ name: "test2", fn: () => {} });
Deno.test("test3", { permissions: "none" }, () => {});
Deno.test({ name: "test4" }, () => {});
Deno.test({ ignore: true }, function test5() {});
