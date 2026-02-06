Deno.describe("suite", () => {
  Deno.test("runs", () => {
    // this runs
  });

  Deno.test.ignore("skipped via ignore", () => {
    throw new Error("should not run");
  });

  Deno.test.ignore("skipped via skip", () => {
    throw new Error("should not run");
  });

  Deno.describe.ignore("skipped suite", () => {
    Deno.test("should not run", () => {
      throw new Error("should not run");
    });
  });
});
