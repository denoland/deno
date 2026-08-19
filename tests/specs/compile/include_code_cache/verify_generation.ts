try {
  Deno.statSync("main-evaluated.txt");
  throw new Error("code cache generation evaluated user code");
} catch (error) {
  if (!(error instanceof Deno.errors.NotFound)) {
    throw error;
  }
}

console.log("code cache generation did not evaluate user code");
