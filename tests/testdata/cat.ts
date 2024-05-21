const filename = Deno.args[0];
using file = await Deno.open(filename);

await file.readable.pipeTo(Deno.stdout.writable);
