const buf: Uint8Array = await Deno.readFile("hello.txt");
const n: number = await Deno.stdout.write(buf);
console.log(`\n\nwrite ${n}`);
