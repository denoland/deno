const worker = new Worker(
  new URL("./read_check_granular_worker.js", import.meta.url).href,
  {
    type: "module",
    deno: {
      namespace: true,
      permissions: {
        read: [],
      },
    },
  },
);

let received = 0;
const messages = [];

worker.onmessage = ({ data: childResponse }) => {
  received++;
  postMessage({
    childHasPermission: childResponse.hasPermission,
    index: childResponse.index,
    parentHasPermission: messages[childResponse.index],
  });
  if (received === messages.length) {
    worker.terminate();
  }
};

onmessage = async ({ data }) => {
  const path = new URL(data.route, import.meta.url);
  const { state } = await Deno.permissions.query({
    name: "read",
    path: path.pathname,
  });

  messages[data.index] = state === "granted";

  worker.postMessage({
    index: data.index,
    route: data.route,
  });
};
