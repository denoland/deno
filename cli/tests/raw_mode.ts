Deno.setRaw(0, true);
Deno.setRaw(0, true); // Can be called multiple times

Deno.stdout.writeSync(new TextEncoder().encode("S"));

const buf = new Uint8Array(3);
for (let i = 0; i < 3; i++) {
  const nread = await Deno.stdin.read(buf);
  if (nread === Deno.EOF) {
    break;
  } else {
    const data = new TextDecoder().decode(buf.subarray(0, nread));
    Deno.stdout.writeSync(new TextEncoder().encode(data.toUpperCase()));
  }
}

Deno.setRaw(0, false); // restores old mode.
Deno.setRaw(0, false); // Can be safely called multiple times
