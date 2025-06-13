const filePath = "./cowsay/package.json";
const packageJson = JSON.parse(Deno.readTextFileSync(filePath));
packageJson.dependencies = {
  "@denotest/add": "1",
};
Deno.writeTextFileSync(filePath, JSON.stringify(packageJson));
