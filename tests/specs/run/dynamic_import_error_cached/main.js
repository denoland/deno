try {
  await import("./a.ts");
} catch {
  console.log("fail");
}
Deno.writeTextFileSync("a.ts", "");
try {
  await import("./a.ts");
} catch {
  console.log("fail");
}
