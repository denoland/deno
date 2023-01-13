new Worker("data:,setTimeout(() => Deno.exit(2), 200)", { type: "module" });

try {
  await new Deno.Command("ps", {
    stdout: "inherit",
    stderr: "inherit",
  }).output();
} catch {
  Deno.exit(0);
}
