import { assertEquals } from "./test_util.ts";

// @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
const { createTestFilter } = Deno[Deno.internal];

Deno.test("filterAsString", function (): void {
  const testFilter = createTestFilter("my-test");
  const tests = [
    {
      fn(): void {},
      name: "my-test",
    },
    {
      fn(): void {},
      name: "other-test",
    },
  ];
  const filteredTests = tests.filter(testFilter);
  assertEquals(filteredTests.length, 1);
});

Deno.test("filterAsREGEX", function (): void {
  const testFilter = createTestFilter("/.+-test/");
  const tests = [
    {
      fn(): void {},
      name: "my-test",
    },
    {
      fn(): void {},
      name: "other-test",
    },
  ];
  const filteredTests = tests.filter(testFilter);
  assertEquals(filteredTests.length, 2);
});

Deno.test("filterAsEscapedREGEX", function (): void {
  const testFilter = createTestFilter("/\\w+-test/");
  const tests = [
    {
      fn(): void {},
      name: "my-test",
    },
    {
      fn(): void {},
      name: "other-test",
    },
  ];
  const filteredTests = tests.filter(testFilter);
  assertEquals(filteredTests.length, 2);
});
