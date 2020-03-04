// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This test is executed as part of tools/test.py
// But it can also be run manually: ./target/debug/deno cli/js/tests/unit_tests.ts

import "./blob.ts";
import "./body.ts";
import "./buffer.ts";
import "./build.ts";
import "./chmod.ts";
import "./chown.ts";
import "./compiler_api.ts";
import "./console.ts";
import "./copy_file.ts";
import "./custom_event.ts";
import "./dir.ts";
import "./dispatch_minimal.ts";
import "./dispatch_json.ts";
import "./error_stack.ts";
import "./event.ts";
import "./event_target.ts";
import "./fetch.ts";
import "./file.ts";
import "./files.ts";
import "./form_data.ts";
import "./format_error.ts";
import "./fs_events.ts";
import "./get_random_values.ts";
import "./globals.ts";
import "./headers.ts";
import "./internals.ts";
import "./link.ts";
import "./location.ts";
import "./make_temp.ts";
import "./metrics.ts";
import "./dom_iterable.ts";
import "./mkdir.ts";
import "./net.ts";
import "./os.ts";
import "./permissions.ts";
import "./process.ts";
import "./realpath.ts";
import "./read_dir.ts";
import "./read_file.ts";
import "./read_link.ts";
import "./remove.ts";
import "./rename.ts";
import "./request.ts";
import "./resources.ts";
import "./signal.ts";
import "./stat.ts";
import "./symbols.ts";
import "./symlink.ts";
import "./text_encoding.ts";
import "./testing.ts";
import "./timers.ts";
import "./tls.ts";
import "./truncate.ts";
import "./tty.ts";
import "./url.ts";
import "./url_search_params.ts";
import "./utime.ts";
import "./write_file.ts";
import "./performance.ts";
import "./version.ts";

if (import.meta.main) {
  await Deno.runTests();
}
