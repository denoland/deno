Deno.stdin.setRaw(true);
Deno.stdin.setRaw(true); // Can be called multiple times

Deno.stdout.writeSync(new TextEncoder().encode("S"));

const buf = new Uint8Array(3);
for (let i = 0; i < 3; i++) {
  const nread = await Deno.stdin.read(buf);
  if (nread === null) {
    break;
  } else {
    const data = new TextDecoder().decode(buf.subarray(0, nread));
    Deno.stdout.writeSync(new TextEncoder().encode(data.toUpperCase()));
  }
}

Deno.stdin.setRaw(false); // restores old mode.
Deno.stdin.setRaw(false); // Can be safely called multiple times
