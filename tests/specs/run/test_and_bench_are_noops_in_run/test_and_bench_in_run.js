Deno.test(function foo() {
});

Deno.test.beforeAll(() => {});
Deno.test.beforeEach(() => {});
Deno.test.afterEach(() => {});
Deno.test.afterAll(() => {});
Deno.test.ignore("ignored", () => {});
Deno.test.only("only", () => {});
Deno.test.sanitizer({ ops: false });
Deno.test.each([1, 2])("each %d", () => {});

Deno.bench(function bar() {
});
