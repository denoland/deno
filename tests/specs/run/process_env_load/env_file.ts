process.loadEnvFile("./env");
console.log(Deno.env.get("FOO"));
