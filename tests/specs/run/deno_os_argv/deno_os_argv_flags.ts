// Deno flags like --allow-read should appear in osArgv but NOT in Deno.args
const osArgv = Deno.osArgv;
console.log("has allow-read:", osArgv.includes("--allow-read"));
console.log("has allow-net:", osArgv.includes("--allow-net"));
console.log("args has no flags:", !Deno.args.includes("--allow-read"));
