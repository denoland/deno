Deno.cron("success-cron", "* * * * *", () => {
  console.log("test-cron executed");
});

// wait to ensure cron sock sees both without race conditions
await new Promise((r) => setTimeout(r, 1000));

Deno.cron("fail-cron", "*/5 * * * *", { backoffSchedule: [100] }, () => {
  throw new Error("an error");
});
