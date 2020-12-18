onmessage = async ({ data }) => {
  const path = new URL(data.route, import.meta.url);
  const { state } = await Deno.permissions.query({
    name: "read",
    path,
  });

  postMessage({
    hasPermission: state === "granted",
    index: data.index,
  });
  close();
};
