onmessage = async ({ data }) => {
  const { state } = await Deno.permissions.query({
    name: "read",
    path: data.path,
  });

  postMessage({
    hasPermission: state === "granted",
    index: data.index,
  });
};
