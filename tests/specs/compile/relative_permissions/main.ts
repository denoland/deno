try {
  Deno.readTextFileSync("a.txt");
} catch (err) {
  console.log(err.message);
}
