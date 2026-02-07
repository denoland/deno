function getSpecifierEsm() {
  return "./sync.mjs";
}

function getSpecifierAmbiguous() {
  return "./sync.js";
}

console.log("Loading...");
require(getSpecifierEsm());
require(getSpecifierAmbiguous());
