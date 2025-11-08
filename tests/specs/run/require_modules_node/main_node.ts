console.log("main_node.ts starts");

if (globalThis.__require_node_worked__) {
  console.log("SUCCESS: require('node:*') worked in --require module");
} else {
  console.log("ERROR: require('node:*') did NOT work");
}

console.log("main_node.ts finished");
