onmessage = async ({ data }) => {
  console.log("%cmessage in 2", "color: blue");
  const path = new URL(data.route, import.meta.url);
  const { state } = await Deno.permissions.query({
    name: "read",
    path: path.pathname,
  });

  console.log("%cmessage out 2", "color: blue");
  postMessage({
    hasPermission: state === "granted",
    index: data.index,
  });
};
