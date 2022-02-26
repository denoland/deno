// Check that we can use the async keyword.
async function main() {
  await new Promise((resolve) => {
    console.log("2");
    setTimeout(resolve, 100);
  });
  console.log("3");
}

console.log("1");
main();
