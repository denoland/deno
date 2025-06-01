// When run against the test HTTP server, it will serve different media types
// based on the URL containing `.t#.` strings, which exercises the different
// mapping of media types end to end.

import { loaded as loadedTs1 } from "https://localhost:5545/subdir/mt_text_typescript.t1.ts";
import { loaded as loadedTs2 } from "https://localhost:5545/subdir/mt_video_vdn.t2.ts";
import { loaded as loadedTs3 } from "https://localhost:5545/subdir/mt_video_mp2t.t3.ts";
import { loaded as loadedTs4 } from "https://localhost:5545/subdir/mt_application_x_typescript.t4.ts";
import { loaded as loadedJs1 } from "https://localhost:5545/subdir/mt_text_javascript.j1.js";
import { loaded as loadedJs2 } from "https://localhost:5545/subdir/mt_application_ecmascript.j2.js";
import { loaded as loadedJs3 } from "https://localhost:5545/subdir/mt_text_ecmascript.j3.js";
import { loaded as loadedJs4 } from "https://localhost:5545/subdir/mt_application_x_javascript.j4.js";

console.log(
  "success",
  loadedTs1,
  loadedTs2,
  loadedTs3,
  loadedTs4,
  loadedJs1,
  loadedJs2,
  loadedJs3,
  loadedJs4,
);
