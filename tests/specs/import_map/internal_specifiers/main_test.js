Deno.test("internal test harness modules are not remapped", () => {
  if (typeof Deno.test !== "function") {
    throw new Error("unreachable");
  }
});
