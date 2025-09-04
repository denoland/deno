Deno.mkdirSync("data");
Deno.writeTextFileSync("data/1.txt", "Hello, world!");
const bytes = new Uint8Array((1024 ** 2) * 20);
for (let i = 0; i < bytes.length; i++) {
  bytes[i] = i % 256;
}
Deno.writeFileSync("data/2.dat", bytes);
