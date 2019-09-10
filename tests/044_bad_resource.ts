async function main(): Promise<void> {
  const file = await Deno.open("Cargo.toml", "r");
  file.close();
  await file.seek(10, 0);
}

main();
