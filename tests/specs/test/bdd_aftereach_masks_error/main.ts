import { afterEach, describe, it } from "@std/testing/bdd";

describe("failure is not apparent", () => {
  it("failing 'it' block", () => {
    throw new Error("original test failure");
  });
  afterEach(() => {
    throw new Error(
      "afterEach failure, perhaps a side-effect of the original failure",
    );
  });
});
