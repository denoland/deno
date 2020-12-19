import { assertEquals, unitTest } from "./test_util.ts";

// @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
const { createFilterFn } = Deno[Deno.internal];

unitTest(function filterAsString(): void {
  const filterFn = createFilterFn("my-test");
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
  const filteredTests = tests.filter(filterFn);
  assertEquals(filteredTests.length, 1);
});

unitTest(function filterAsREGEX(): void {
  const filterFn = createFilterFn("/.+-test/");
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
  const filteredTests = tests.filter(filterFn);
  assertEquals(filteredTests.length, 2);
});

unitTest(function filterAsEscapedREGEX(): void {
  const filterFn = createFilterFn("/\\w+-test/");
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
  const filteredTests = tests.filter(filterFn);
  assertEquals(filteredTests.length, 2);
});
