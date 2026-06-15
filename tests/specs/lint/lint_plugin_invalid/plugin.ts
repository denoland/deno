// A module that loads fine but is not a valid Deno lint plugin (e.g. an ESLint
// plugin). It is missing the required string `name`, which previously produced
// an anonymous "Linter plugin name must be a string" error that didn't say
// which plugin was at fault.
export default {
  rules: {},
};
