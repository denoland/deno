Deno.test(function throws() {
  // Leak
  setTimeout(() => {}, 60_000);
  // But the exception should mask the leak
  throw new Error("Throws");
});
