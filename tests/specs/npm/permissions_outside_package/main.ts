import { loadConfigFile } from "npm:@denotest/permissions-outside-package";

const fileName = `${Deno.cwd()}/foo/config.js`;
const config = loadConfigFile(fileName);
console.log(config);
