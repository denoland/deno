import { loadConfigFile } from "npm:@denotest/permissions-outside-package";

const url = import.meta.resolve("./foo/config.js");
const config = loadConfigFile(url.slice(7));
console.log(config);
