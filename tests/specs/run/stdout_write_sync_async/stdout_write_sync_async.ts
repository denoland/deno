const encoder = new TextEncoder();
const pending = [];

for (let i = 0; i < 100; i++) {
  // some code that will cause stdout to be written
  // synchronously while the async write might be occurring
  console.log("Hello");
  pending.push(Deno.stdout.write(encoder.encode("Hello\n")));
  if (i % 10) {
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
}

await Promise.all(pending);
