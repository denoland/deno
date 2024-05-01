import { setValue } from "@denotest/esm-basic";
setValue(5);

const packageJson = JSON.parse(await Deno.readTextFile("./package.json"));

if (
  !packageJson.dependencies || !packageJson.dependencies["@denotest/esm-basic"]
) {
  throw new Error("Package json missing dep!");
}
