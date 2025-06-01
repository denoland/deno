export async function myFunction() {
  await new Promise((resolve) =>
    setTimeout(() => {
      postMessage("hallo");
      resolve;
    }, 100)
  );
}
await myFunction();
