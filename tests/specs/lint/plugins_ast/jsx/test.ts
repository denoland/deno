import plugin from "./plugin.ts";
const fileName = "source.tsx";
const sourceText = await Deno.readTextFile(`./${fileName}`);

Deno.test(function testTs() {
  Deno[Deno.internal].runLintPlugin(plugin, fileName, sourceText);
});
