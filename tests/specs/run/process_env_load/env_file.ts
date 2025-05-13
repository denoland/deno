import { loadEnvFile } from "node:process";
process.loadEnvFile("./env");
loadEnvFile("./env");
console.log(Deno.env.get("FOO"));
