const name = Deno.args[0].trim();

function exists(path: string) {
  try {
    Deno.statSync(path);
    return true;
  } catch (error) {
    return false;
  }
}

if (
  !(exists(`node_modules/.bin/${name}`) ||
    exists(`node_modules/.bin/${name}.cmd`))
) {
  console.log("missing bin");
  console.log(`node_modules/.bin/${name}`);
  console.log(Deno.readDirSync("node_modules/.bin").toArray());
  Deno.exit(1);
}
