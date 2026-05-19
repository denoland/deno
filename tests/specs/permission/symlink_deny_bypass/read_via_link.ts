try {
  const content = Deno.readTextFileSync("allowed/link");
  console.log("read bypass succeeded (BUG):", content);
  Deno.exit(1);
} catch (e) {
  if (e instanceof Deno.errors.NotCapable) {
    console.log("read correctly denied");
  } else {
    throw e;
  }
}
