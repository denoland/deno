async function main(): Promise<void> {
  const file = await Deno.open("044_bad_resource.ts", "r");
  file.close();
  await file.seek(10, 0);
}

main();
