const reader = Deno.stdin.readable.getReader();

setTimeout(async () => {
  await reader.cancel("done");
  console.log("CANCELLED");
}, 250);

try {
  await reader.read();
} catch {
  // The pending read is interrupted by canceling the reader.
}

await new Promise((resolve) => setTimeout(resolve, 0));
console.log("DONE");
