new Worker("data:,setTimeout(() => Deno.exit(2), 200)", { type: "module" });

try {
  await Deno.spawn("ps", {
    stdout: "inherit",
    stderr: "inherit",
  });
} catch {
  Deno.exit(0);
}
