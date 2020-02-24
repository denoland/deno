const path = await Deno.makeTempFile({ dir: "." });
console.log(path);
await Deno.remove(path);
