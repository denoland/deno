// A plugin loaded via deno.json config
export default {
  name: "config-plugin",

  resolveId(source: string, _importer: string | null, _options: unknown) {
    if (source.includes("entry.ts")) {
      console.log(`[config-plugin] resolving: ${source}`);
    }
    return null;
  },

  load(id: string) {
    console.log(`[config-plugin] loading: ${id}`);
    return null;
  },
};
