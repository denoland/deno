export default {
  name: "test-plugin",
  rules: {
    "my-rule": {
      create() {
        // The plugin host doesn't run its event loop after linting a file, so
        // this work is still pending when `deno lint` shuts the host down.
        setTimeout(() => {}, 1_000_000);
        setInterval(() => {}, 1_000);
        Deno.readTextFile(import.meta.filename!);
        return {};
      },
    },
  },
};
