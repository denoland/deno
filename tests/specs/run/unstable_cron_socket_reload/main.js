console.error("[ISOLATE] Starting");

// Register a cron job
console.error("[ISOLATE] Registering test-cron");
Deno.cron("test-cron", "* * * * *", () => {
  console.error("[ISOLATE] test-cron executed");
});
console.error("[ISOLATE] test-cron registered successfully");

// Keep alive with Deno.serve
Deno.serve(() => new Response("Hello"));
