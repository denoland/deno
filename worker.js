const file = Deno.openSync("./clearscreen.txt");
file.readable.pipeTo(Deno.stdout.writable);

// console.clear();
console.log("Are you sure you want to continu?");
