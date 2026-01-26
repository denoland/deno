// A simple custom plugin for testing
export default {
  name: "test-custom-plugin",

  resolveId(source: string, _importer: string | null, _options: unknown) {
    // Log when we resolve
    if (source.includes("main.ts")) {
      console.log(`[test-plugin] resolving: ${source}`);
    }
    return null; // Let Deno handle resolution
  },

  load(id: string) {
    // Log when we load
    console.log(`[test-plugin] loading: ${id}`);
    return null; // Let Deno handle loading
  },
};
