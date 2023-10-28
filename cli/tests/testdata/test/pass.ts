Deno.test("test 0", () => {});
Deno.test("test 1", () => {});
Deno.test("test 2", () => {});
Deno.test("test 3", () => {});
Deno.test("test 4", () => {});
Deno.test("test 5", () => {});
Deno.test("test 6", () => {});
Deno.test("test 7", () => {});
Deno.test("test 8", () => {
  console.log("console.log");
});
Deno.test("test 9", () => {
  console.error("console.error");
});

Deno.test("test\b", () => {
  console.error("console.error");
});
Deno.test("test\f", () => {
  console.error("console.error");
});

Deno.test("test\t", () => {
  console.error("console.error");
});

Deno.test("test\n", () => {
  console.error("console.error");
});

Deno.test("test\r", () => {
  console.error("console.error");
});

Deno.test("test\v", () => {
  console.error("console.error");
});
