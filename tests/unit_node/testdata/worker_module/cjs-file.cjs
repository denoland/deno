const { add } = require("./other_cjs_file.cjs");

const missing_toplevel_async = async () => {
  return new Promise((resolve) => {
    setTimeout(() => {
      resolve;
    }, 500);
  });
};

async function main() {
  /// async code doesn't seem to work within this CJS wrapper :(
  //const p = await missing_toplevel_async();

  const sum = add(2, 3);
  if (sum != 5) {
    throw ("Bad calculator!");
  }

  postMessage("hallo");
}

main();
