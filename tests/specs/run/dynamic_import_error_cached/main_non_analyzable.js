const file = "./a" + ".ts";
try {
  await import(file);
} catch {
  console.log("fail");
}
Deno.writeTextFileSync("a.ts", "");
try {
  await import(file);
} catch {
  console.log("fail");
}
