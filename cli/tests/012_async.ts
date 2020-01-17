// Check that we can use the async keyword.
async function main(): Promise<void> {
  await new Promise((resolve): void => {
    console.log("2");
    setTimeout(resolve, 100);
  });
  console.log("3");
}

console.log("1");
main();
