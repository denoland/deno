Deno.cron("A fun cron 123 - _", "* * * * *", async () => {
  await new Promise((r) => setTimeout(r, 10));
  console.log("test-cron executed");
});

// wait to ensure cron sock sees both without race conditions
await new Promise((r) => setTimeout(r, 1000));

Deno.cron("Fail cron", "*/5 * * * *", { backoffSchedule: [100] }, async () => {
  await new Promise((r) => setTimeout(r, 10));
  throw new Error("an error");
});
