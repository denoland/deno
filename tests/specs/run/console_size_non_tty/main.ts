try {
  Deno.consoleSize();
  console.log("unexpectedly returned a size");
} catch (e) {
  console.log(e.message);
}
