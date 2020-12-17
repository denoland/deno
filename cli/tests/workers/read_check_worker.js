onmessage = async () => {
  const {state} = await Deno.permissions.query({
    name: "read",
  });
  postMessage(state === "granted");
  close();
};