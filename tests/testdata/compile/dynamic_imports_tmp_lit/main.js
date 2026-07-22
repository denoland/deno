const fileNames = [
  "a.js",
  "b.ts",
];

for (const fileName of fileNames) {
  await import(`./sub/${fileName}`);
}

const jsonFileNames = ["data.json", "sub/data2.json"];
for (const fileName of jsonFileNames) {
  const mod = await import(`./other/${fileName}`, { with: { type: "json" } });
  console.log(mod.default);
}
