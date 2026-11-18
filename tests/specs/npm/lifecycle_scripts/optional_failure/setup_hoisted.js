const dependencies = {
  "@denotest/optional-lifecycle": `${Deno.args[0]}.0.0`,
};
if (Deno.args[1] === "required") {
  dependencies["@denotest/failing-lifecycle"] = "1.0.0";
}
Deno.writeTextFileSync("package.json", JSON.stringify({ dependencies }));
Deno.copyFileSync("hoisted.json", "deno.json");
