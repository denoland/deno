try {
  await Deno.loadSystemFonts();
  console.log("ERROR: should have thrown");
} catch (e) {
  console.log(e.constructor.name + ": " + e.message);
}
