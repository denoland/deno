import "./main.ts";

Deno.bench("bench", () => {
  const testing = 1 + 2;
  if (testing !== 3) {
    throw "FAIL";
  }
});
