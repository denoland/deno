import { fromFileUrl } from "../../../test_util/std/path/mod.ts";

onmessage = async ({ data }) => {
  const { state } = await Deno.permissions.query({
    name: "read",
    path: fromFileUrl(new URL(data.route, import.meta.url)),
  });

  postMessage({
    hasPermission: state === "granted",
    index: data.index,
  });
};
