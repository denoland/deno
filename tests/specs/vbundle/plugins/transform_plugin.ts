// A plugin that transforms code
export default {
  name: "test-transform-plugin",
  // Declare that we handle .ts files so the VFS knows to call our transform hook
  extensions: [".ts"],

  transform(code: string, id: string) {
    if (!id.endsWith(".ts") && !id.endsWith(".js")) {
      return null;
    }

    // Check if code contains DEBUG
    if (code.includes("DEBUG")) {
      console.log(`[transform-plugin] transforming: ${id}`);
      // Replace DEBUG with false for "production" build
      const transformed = code.replace(/const DEBUG = true/g, "const DEBUG = false");
      return {
        code: transformed,
        map: undefined,
      };
    }

    return null;
  },
};
