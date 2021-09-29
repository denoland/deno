onmessage = async ({ names }) => {
  const permissions = [];
  for (const name of names) {
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
