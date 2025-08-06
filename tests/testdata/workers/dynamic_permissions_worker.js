onmessage = async ({ data }) => {
  const results = {};
  for (const [permType, paths] of Object.entries(data.permissions)) {
    results[permType] = {};
    for (const path of paths) {
      try {
        const { state } = await Deno.permissions.query({
          name: permType,
          path: path,
        });
        results[permType][path] = state;
      } catch (error) {
        results[permType][path] = "error: " + error.message;
      }
    }
  }
  postMessage({ type: "permissionResults", results });
};
