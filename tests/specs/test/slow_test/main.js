Deno.test(async function test() {
  // We want to get at least one slow test warning
  await new Promise((r) => setTimeout(r, 3_000));
});
