const buf = await Deno.readFile("hello.txt");
const n = await Deno.stdout.write(buf);
console.log(`\n\nwrite ${n}`);
