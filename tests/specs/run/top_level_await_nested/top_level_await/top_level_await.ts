const buf: Uint8Array = await Deno.readFile("./assets/hello.txt");
const n: number = await Deno.stdout.write(buf);
console.log(`\n\nwrite ${n}`);
