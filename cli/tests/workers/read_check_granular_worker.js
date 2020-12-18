onmessage = async ({ data }) => {
  const path = new URL(data, import.meta.url);
  const { state } = await Deno.permissions.query({
    name: "read",
    path,
  });

  postMessage(state === "granted");
  close();
};
