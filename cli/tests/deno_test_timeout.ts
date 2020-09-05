function foo() {
  return new Promise((_resolve, _reject) => {
    // neither resolve nor reject is called
  });
}

Deno.test({
  name: "ever-pending promise",
  async fn() {
    await foo();
  },
  timeout: 500,
});
