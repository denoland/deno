import { loadConfigFile } from "npm:@denotest/permissions-outside-package";

const fileName = `${Deno.cwd()}/permissions_outside_package/foo/config.js`;
const config = loadConfigFile(fileName);
console.log(config);
