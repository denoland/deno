onmessage = async ({ data }) => {
  const path = new URL(data.route, import.meta.url);
  const { state } = await Deno.permissions.query({
    name: "read",
    path: path.pathname,
  });

  postMessage({
    hasPermission: state === "granted",
    index: data.index,
  });
};
