const readVersion = (p: string) =>
  JSON.parse(Deno.readTextFileSync(p + "/package.json")).version;

const exists = (p: string) => {
  try {
    Deno.statSync(p);
    return true;
  } catch {
    return false;
  }
};

const top = "node_modules/@denotest/different-nested-dep-child";
const chainTop = "node_modules/@denotest/hoisted-nested-grandchild-chain";
const parentTop = "node_modules/@denotest/hoisted-nested-grandchild-parent";
const nestedChain =
  `${parentTop}/node_modules/@denotest/hoisted-nested-grandchild-chain`;
const nestedGrandchild =
  `${nestedChain}/node_modules/@denotest/different-nested-dep-child`;
const wrongPlacement =
  `${chainTop}/node_modules/@denotest/different-nested-dep-child`;

console.log("top child:", readVersion(top));
console.log("top chain:", readVersion(chainTop));
console.log("nested chain:", readVersion(nestedChain));
console.log("nested grandchild:", readVersion(nestedGrandchild));
console.log("wrong placement exists:", exists(wrongPlacement));
