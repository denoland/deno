import "./main.ts";

Deno.test("test", () => {
  const testing = 1 + 2;
  if (testing !== 3) {
    throw "FAIL";
  }
});
