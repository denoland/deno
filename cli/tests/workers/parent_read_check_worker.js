onmessage = async () => {
  const { state } = await Deno.permissions.query({
    name: "read",
  });

  const worker = new Worker(
    new URL("./read_check_worker.js", import.meta.url).href,
    {
      type: "module",
      deno: {
        namespace: true,
        permissions: {
          read: false,
        },
      },
    },
  );

  worker.onmessage = ({ data: childHasPermission }) => {
    postMessage({
      parentHasPermission: state === "granted",
      childHasPermission,
    });
    close();
  };
  worker.postMessage(null);
};
