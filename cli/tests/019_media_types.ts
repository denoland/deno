// When run against the test HTTP server, it will serve different media types
// based on the URL containing `.t#.` strings, which exercises the different
// mapping of media types end to end.

import { loaded as loadedTs1 } from "http://localhost:4545/cli/tests/subdir/mt_text_typescript.t1.ts";
import { loaded as loadedTsx1 } from "http://localhost:4545/cli/tests/subdir/mt_text_typescript_tsx.t1.tsx";
import { loaded as loadedTs2 } from "http://localhost:4545/cli/tests/subdir/mt_video_vdn.t2.ts";
import { loaded as loadedTsx2 } from "http://localhost:4545/cli/tests/subdir/mt_video_vdn_tsx.t2.tsx";
import { loaded as loadedTs3 } from "http://localhost:4545/cli/tests/subdir/mt_video_mp2t.t3.ts";
import { loaded as loadedTsx3 } from "http://localhost:4545/cli/tests/subdir/mt_video_mp2t_tsx.t3.tsx";
import { loaded as loadedTs4 } from "http://localhost:4545/cli/tests/subdir/mt_application_x_typescript.t4.ts";
import { loaded as loadedTsx4 } from "http://localhost:4545/cli/tests/subdir/mt_application_x_typescript_tsx.t4.tsx";
import { loaded as loadedJs1 } from "http://localhost:4545/cli/tests/subdir/mt_text_javascript.j1.js";
import { loaded as loadedJsx1 } from "http://localhost:4545/cli/tests/subdir/mt_text_javascript_jsx.j1.jsx";
import { loaded as loadedJs2 } from "http://localhost:4545/cli/tests/subdir/mt_application_ecmascript.j2.js";
import { loaded as loadedJsx2 } from "http://localhost:4545/cli/tests/subdir/mt_application_ecmascript_jsx.j2.jsx";
import { loaded as loadedJs3 } from "http://localhost:4545/cli/tests/subdir/mt_text_ecmascript.j3.js";
import { loaded as loadedJsx3 } from "http://localhost:4545/cli/tests/subdir/mt_text_ecmascript_jsx.j3.jsx";
import { loaded as loadedJs4 } from "http://localhost:4545/cli/tests/subdir/mt_application_x_javascript.j4.js";
import { loaded as loadedJsx4 } from "http://localhost:4545/cli/tests/subdir/mt_application_x_javascript_jsx.j4.jsx";

console.log(
  "success",
  loadedTs1,
  loadedTsx1,
  loadedTs2,
  loadedTsx2,
  loadedTs3,
  loadedTsx3,
  loadedTs4,
  loadedTsx4,
  loadedJs1,
  loadedJsx1,
  loadedJs2,
  loadedJsx2,
  loadedJs3,
  loadedJsx3,
  loadedJs4,
  loadedJsx4
);
