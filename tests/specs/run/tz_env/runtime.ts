Deno.env.set("TZ", "Asia/Manila");
const d = new Date("2020-06-26T00:00:00Z");
console.log(d.getHours());
console.log(d.toString());
