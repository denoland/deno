let testCount = 0;
const logs: string[] = [];

Deno.test.beforeEach(() => {
  testCount++;
  logs.push(`beforeEach executed for test ${testCount}`);
});

Deno.test.afterEach(() => {
  logs.push(`afterEach2 executed for test ${testCount}`);
});

Deno.test.afterEach(() => {
  logs.push(`afterEach executed for test ${testCount}`);

  if (testCount === 1) {
    throw new Error("afterEach hook failed on second test");
  }
});

Deno.test("first test", () => {
  logs.push("first test executed");
});

Deno.test("second test", () => {
  logs.push("second test executed");
});

globalThis.onunload = () => {
  console.log(logs);
};
