// When run against the test HTTP server, it will serve different media types
// based on the URL containing `.t#.` strings, which exercises the different
// mapping of media types end to end.
import { loaded as loadedTsx1 } from "http://localhost:4545/cli/tests/subdir/mt_text_typescript_tsx.t1.tsx";
import { loaded as loadedTsx2 } from "http://localhost:4545/cli/tests/subdir/mt_video_vdn_tsx.t2.tsx";
import { loaded as loadedTsx3 } from "http://localhost:4545/cli/tests/subdir/mt_video_mp2t_tsx.t3.tsx";
import { loaded as loadedTsx4 } from "http://localhost:4545/cli/tests/subdir/mt_application_x_typescript_tsx.t4.tsx";
import { loaded as loadedJsx1 } from "http://localhost:4545/cli/tests/subdir/mt_text_javascript_jsx.j1.jsx";
import { loaded as loadedJsx2 } from "http://localhost:4545/cli/tests/subdir/mt_application_ecmascript_jsx.j2.jsx";
import { loaded as loadedJsx3 } from "http://localhost:4545/cli/tests/subdir/mt_text_ecmascript_jsx.j3.jsx";
import { loaded as loadedJsx4 } from "http://localhost:4545/cli/tests/subdir/mt_application_x_javascript_jsx.j4.jsx";

declare global {
  // deno-lint-ignore no-namespace
  namespace JSX {
    interface IntrinsicElements {
      // deno-lint-ignore no-explicit-any
      [elemName: string]: any;
    }
  }
}

console.log(
  "success",
  loadedTsx1,
  loadedTsx2,
  loadedTsx3,
  loadedTsx4,
  loadedJsx1,
  loadedJsx2,
  loadedJsx3,
  loadedJsx4,
);
