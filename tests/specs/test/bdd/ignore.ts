Deno.describe("suite", () => {
  Deno.it("runs", () => {
    // this runs
  });

  Deno.it.ignore("skipped via ignore", () => {
    throw new Error("should not run");
  });

  Deno.it.skip("skipped via skip", () => {
    throw new Error("should not run");
  });

  Deno.describe.skip("skipped suite", () => {
    Deno.it("should not run", () => {
      throw new Error("should not run");
    });
  });
});
