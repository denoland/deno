const encoder = new TextEncoder();

const pending = [];

// do this a bunch of times to ensure it doesn't race
// and everything happens in order
for (let i = 0; i < 50; i++) {
  const buf = new Uint8Array(1);
  pending.push(
    Deno.stdin.read(buf).then(() => {
      return Deno.stdout.write(buf);
    }),
  );
}

await Promise.all(pending);
await Deno.stdout.write(encoder.encode("\n"));
