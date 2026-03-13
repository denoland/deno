console.error("[ISOLATE] Starting");

// 1. Register cron BEFORE Deno.serve
console.error("[ISOLATE] Registering early-cron (before serve)");
Deno.cron("early-cron", "* * * * *", () => {
  console.error("[ISOLATE] early-cron executed");
});
console.error("[ISOLATE] early-cron registered successfully");

// 2. Start HTTP server
Deno.serve({ port: 0 }, () => new Response("ok"));
console.error("[ISOLATE] Deno.serve started");

// 3. Wait for rejection message to be processed
await new Promise((resolve) => setTimeout(resolve, 1500));

// 4. Try to register cron AFTER Deno.serve
try {
  Deno.cron("late-cron", "* * * * *", () => {
    console.error("[ISOLATE] late-cron executed (should never happen)");
  });
  console.error("[ISOLATE] ERROR: late-cron should have been rejected");
  Deno.exit(1);
} catch (error) {
  console.error(
    "[ISOLATE] late-cron rejected as expected with error:",
    error.message,
  );
}
