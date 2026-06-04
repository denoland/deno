const packageJsonText = await Deno.readTextFile(
  "npm:@denotest/esm-basic@1.0.0/package.json",
);
const packageJson = JSON.parse(packageJsonText);
console.log(packageJson.name);

const mainBytes = await Deno.readFile(
  "npm:@denotest/esm-basic@1.0.0/main.mjs",
);
const mainText = new TextDecoder().decode(mainBytes);
console.log(mainText.includes("getValue"));

const packageJsonTextSync = Deno.readTextFileSync(
  "npm:@denotest/esm-basic@1.0.0/package.json",
);
console.log(JSON.parse(packageJsonTextSync).version);
