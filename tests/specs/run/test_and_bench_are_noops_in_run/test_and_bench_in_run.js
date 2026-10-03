Deno.test(function foo() {
});

Deno.test.beforeAll(() => {});
Deno.test.beforeEach(() => {});
Deno.test.afterEach(() => {});
Deno.test.afterAll(() => {});
Deno.test.ignore("ignored", () => {});
Deno.test.skip("skipped", () => {});
Deno.test.only("only", () => {});
Deno.test.sanitizer({ ops: false });
Deno.test.each([1, 2])("each %d", () => {});
Deno.test.skip.each([1, 2])("skip each %d", () => {});

if (Deno.test.skip !== Deno.test.ignore) {
  throw new Error("Deno.test.skip must alias Deno.test.ignore");
}
if (Deno.test.skip.each !== Deno.test.ignore.each) {
  throw new Error("Deno.test.skip.each must alias Deno.test.ignore.each");
}

Deno.bench(function bar() {
});
