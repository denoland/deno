const NAME = "server";
const root = Deno.env.get("INIT_CWD")!;
console.log(`${NAME} ready`);
await Deno.writeTextFile(`${root}/marker.${NAME}`, "");
const deadline = Date.now() + 15_000;
while (true) {
  try {
    await Deno.stat(`${root}/marker.server`);
    await Deno.stat(`${root}/marker.web`);
    await Deno.stat(`${root}/marker.shared`);
    break;
  } catch {
    if (Date.now() > deadline) {
      console.error(`${NAME}: timed out waiting for siblings`);
      Deno.exit(1);
    }
    await new Promise((r) => setTimeout(r, 30));
  }
}
console.log(`${NAME} done`);
