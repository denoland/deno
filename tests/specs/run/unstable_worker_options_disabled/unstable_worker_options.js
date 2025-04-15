const scope = import.meta.url.slice(-7) === "#worker" ? "worker" : "main";

new Worker(`data:application/javascript;base64,${btoa(`postMessage("ok");`)}`, {
  type: "module",
  deno: {
    permissions: {
      read: true,
    },
  },
}).onmessage = ({ data }) => {
  console.log(scope, data);

  if (scope === "main") {
    const worker = new Worker(`${import.meta.url}#worker`, { type: "module" });
    worker.onmessage = () => Deno.exit(0);
  } else {
    postMessage("done");
  }
};
