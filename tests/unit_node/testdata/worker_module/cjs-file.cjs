const missing_toplevel_async = async () => {
  await new Promise((resolve) =>
    setTimeout(() => {
      postMessage("hallo");
      resolve;
    }, 100)
  );
};

function main() {
  missing_toplevel_async();
}

main();
