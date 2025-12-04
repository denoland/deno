// Test file to verify the code property is accessible on Deno.errors.NotFound
try {
  Deno.readTextFileSync("doesnt-exist");
} catch (e) {
  if (e instanceof Deno.errors.NotFound) {
    // This should now compile without TypeScript errors
    console.log("Error code:", e.code);
    console.log("Error message:", e.message);
  } else {
    throw e;
  }
}
