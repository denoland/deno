import test from "node:test";

test("First Test", () => {
  console.log("Inside first test");
});

test("Second Test", async (t) => {
  t.beforeEach(() => {
    console.log("before each inside second test");
  });

  t.before(() => {
    console.log("before inside second test");
  });

  t.afterEach(() => {
    console.log("after each inside second test");
  });

  t.after(() => {
    console.log("after inside second test");
  });

  await t.test("Nested Test", () => {
    console.log("Inside of the second test's nested test");
  });

  await t.test("Another Nested Test", () => {
    console.log("Inside of the second test's another nested test");
  });
});

test.beforeEach(() => {
  console.log("before each");
});

test.before(() => {
  console.log("before");
});

test.afterEach(() => {
  console.log("after each");
});

test.after(() => {
  console.log("after");
});
