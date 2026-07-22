Deno.test({
  name: "completes well before deadline",
  timeout: 1000,
  async fn() {
    await new Promise((resolve) => setTimeout(resolve, 10));
  },
});
