import plugin from "./plugin.ts";
const fileName = "source.ts";
const sourceText = await Deno.readTextFile(`./${fileName}`);

Deno.test(function testTs() {
  Deno[Deno.internal].runLintPlugin(plugin, fileName, sourceText);
});
