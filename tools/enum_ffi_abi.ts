/** Enumerate all ABIs of all architechures
 * Usage: deno run --unstable --allow-read tools/enum_ffi_abi.ts path/to/libffi/src
 */

import {walkSync, readFileStrSync} from "https://deno.land/std/fs/mod.ts"
import {basename} from "https://deno.land/std/path/mod.ts"

const allABI = new Set()

for (const entry of walkSync(Deno.args[0])) {
    if (basename(entry.path) === "ffitarget.h") {
        const content = readFileStrSync(entry.path)
        const match = /typedef enum ffi_abi\s+\{(.*)\}/s.exec(content)
        if (match) {
            for (const line of match[1].trim().split('\n')) {
                const match = /FFI_\w+/.exec(line)
                if (match) {
                    allABI.add(match[0])
                }
            }
        } else {
            console.error(`Invalid format: ${entry.path}`)
        }

    }
}

allABI.delete("FFI_FIRST_ABI")
allABI.delete("FFI_LAST_ABI")

for (const abi of allABI) {
    console.log(abi)
}
