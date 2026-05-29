const readVersion = (p: string) =>
  JSON.parse(Deno.readTextFileSync(p + "/package.json")).version;

const topX = "node_modules/@denotest/hoisted-walkup-x";
const topChain = "node_modules/@denotest/hoisted-walkup-chain";
const needsX2 = "node_modules/@denotest/hoisted-walkup-needs-x2";
const nestedX2 = `${needsX2}/node_modules/@denotest/hoisted-walkup-x`;
const nestedChain1 = `${nestedX2}/node_modules/@denotest/hoisted-walkup-chain`;
const chainNestedX1 = `${nestedChain1}/node_modules/@denotest/hoisted-walkup-x`;

console.log("top x:", readVersion(topX));
console.log("top chain:", readVersion(topChain));
console.log("nested x@2:", readVersion(nestedX2));
console.log("nested chain@1:", readVersion(nestedChain1));
console.log("chain's nested x:", readVersion(chainNestedX1));
