import process from "node:process";

const date = new Date("2018-04-14T12:34:56.789Z");
Deno.env.set("TZ", "Europe/London");
if (!date.toString().match(/^Sat Apr 14 2018 13:34:56 GMT\+0100 \(.+\)$/)) {
  throw new Error(`date.toString() did not match the expected pattern`);
}

process.loadEnvFile("./timezone.env");
if (!date.toString().match(/^Sat Apr 14 2018 14:34:56 GMT\+0200 \(.+\)$/)) {
  throw new Error(`process.loadEnvFile() did not update the timezone`);
}

console.log("ok");
