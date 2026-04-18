import { describe, it, test } from "node:test";

describe.only("a suite", () => {
  it("a test", () => {});
});

test.only("a test only", () => {});

it.only("an it only", () => {});

// This should be filtered out by the "only" tests above
test("should not run", () => {
  throw new Error("this test should have been filtered out");
});
