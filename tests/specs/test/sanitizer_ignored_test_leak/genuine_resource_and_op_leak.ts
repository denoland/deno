// Verify the ignorelist for sanitizer-ignoring tests does NOT suppress genuine
// resource and async op leaks introduced by a later, sanitized test.

let ignoredListener: Deno.Listener;
let ignoredAccept: Promise<void>;

Deno.test(
  { sanitizeOps: false, sanitizeResources: false },
  function ignoresSanitizersAndLeaksActivities() {
    ignoredListener = Deno.listen({ hostname: "127.0.0.1", port: 0 });
    ignoredAccept = ignoredListener.accept().then(
      (conn) => conn.close(),
      () => {},
    );
  },
);

Deno.test(async function laterTestLeaksItsOwnResourceAndOp() {
  ignoredListener.close();
  await ignoredAccept;

  const listener = Deno.listen({ hostname: "127.0.0.1", port: 0 });
  listener.accept().catch(() => {});
});
