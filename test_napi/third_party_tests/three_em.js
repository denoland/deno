const three_em = Deno.core.dlopen(
  "node_modules/@three-em/node-darwin-arm64/three_em_node.darwin-arm64.node"
);

console.log(await three_em.executeContract("t9T7DIOGxx4VWXoCEeYYarFYeERTpWIC1V3y-BPZgKE"))
