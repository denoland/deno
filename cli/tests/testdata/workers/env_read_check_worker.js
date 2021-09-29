onmessage = async ({ data }) => {
  const permissions = [];
  for (const name of data.names) {
    const { state } = await Deno.permissions.query({
      name: "env",
      variable: name,
    });
    permissions.push(state === "granted");
  }

  postMessage({
    permissions,
  });
};
