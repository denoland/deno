const worker = new Worker(
  new URL("./read_check_granular_worker.js", import.meta.url).href,
  {
    type: "module",
    deno: {
      namespace: true,
      permissions: "none",
    },
  },
);

onmessage = ({ data }) => {
  worker.postMessage(data);
};

worker.onmessage = ({ data }) => {
  postMessage(data);
};
