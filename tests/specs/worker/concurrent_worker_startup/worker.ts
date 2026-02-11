// Import npm packages with large dependency trees to create real
// module loading pressure. Each package triggers many transitive
// module loads, each going through the code cache SQLite mutex.
import "npm:chalk@4";
import "npm:react@18.2";
import "npm:preact@10.19";
import "npm:ajv@8";
import "npm:has@1";
import "npm:picocolors@1";

self.postMessage("ready");
