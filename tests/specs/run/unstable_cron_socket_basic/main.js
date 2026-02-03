console.error("[ISOLATE] Registering test-cron");
Deno.cron("test-cron", "* * * * *", () => {
  console.error("[ISOLATE] test-cron executed");
});

console.error("[ISOLATE] Registering other-cron");
Deno.cron("other-cron", "*/5 * * * * *", () => {
  console.error("[ISOLATE] other-cron executed");
});

console.error("[ISOLATE] isolate booted");

console.error("[ISOLATE] Waiting for execution...");
await new Promise({ port: 0 }, (resolve) => setTimeout(resolve, 2000));
console.error("[ISOLATE] Exiting");
