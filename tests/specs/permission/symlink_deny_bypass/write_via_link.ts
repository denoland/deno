try {
  Deno.writeTextFileSync("allowed/link", "OVERWRITTEN");
  console.log("write bypass succeeded (BUG)");
  Deno.exit(1);
} catch (e) {
  if (e instanceof Deno.errors.NotCapable) {
    console.log("write correctly denied");
  } else {
    throw e;
  }
}
