import { registerHooks } from "node:module";

registerHooks({
  load(url, context, nextLoad) {
    if (url.endsWith("data.json")) {
      console.log(
        "hook:load importAttributes",
        JSON.stringify(context.importAttributes),
      );
    }
    return nextLoad(url, context);
  },
});
