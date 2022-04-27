new Worker("data:,setTimeout(() => Deno.exit(2), 200)", {
  type: "module",
  deno: { namespace: true },
});

try {
  await Deno.spawn("ps");
} catch {
  Deno.exit(0);
}
