

async function main() {
  let r = await Deno.stat("README.md");
  console.log("stat", r);
}


main();
