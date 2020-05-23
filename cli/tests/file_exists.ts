try {
  Deno.openSync(Deno.args[0]);
  Deno.exit(0);
} catch(e) {
  Deno.exit(1);
}
