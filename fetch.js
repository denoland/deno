// const p = await fetch("http://localhost:8000/sample.txt");
const p = await fetch("https://deno.land");
const a = p.text()
gc()
console.log(await a)
