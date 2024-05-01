import { setValue } from "@denotest/esm-basic";
setValue(5);

const denoJson = JSON.parse(await Deno.readTextFile("./deno.json"));
if (!denoJson.imports || !denoJson.imports["@denotest/esm-basic"]) {
  throw new Error("deno.json missing dep!");
}
