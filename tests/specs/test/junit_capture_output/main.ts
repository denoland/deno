Deno.test("captures stdout and stderr", () => {
  console.log("log line");
  console.error("error line");
});

Deno.test("attributes output to steps", async (t) => {
  console.log("before step");
  await t.step("inner step", () => {
    console.log("inside step");
    console.error("step error");
  });
  console.log("after step");
});

Deno.test("no output", () => {});
