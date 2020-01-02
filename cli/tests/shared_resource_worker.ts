onmessage = (): void => {
  // workerClose can only work as intended in onmessage.
  workerClose();
};

console.log(Deno.resources());
