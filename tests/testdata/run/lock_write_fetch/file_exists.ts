try {
  await Deno.open(Deno.args[0]);
  Deno.exit(0);
} catch (_e) {
  Deno.exit(1);
}
