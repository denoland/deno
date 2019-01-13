// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// This test is executed as part of tools/test.py
// But it can also be run manually: ./target/debug/deno js/unit_tests.ts

import "./blob_test.ts";
import "./buffer_test.ts";
import "./chmod_test.ts";
import "./compiler_test.ts";
import "./console_test.ts";
import "./copy_file_test.ts";
import "./dir_test.ts";
import "./event_test.ts";
import "./event_target_test.ts";
import "./fetch_test.ts";
// TODO The global "File" has been temporarily disabled. See the note in
// js/globals.ts
// import "./file_test.ts";
import "./files_test.ts";
import "./form_data_test.ts";
import "./headers_test.ts";
import "./make_temp_dir_test.ts";
import "./metrics_test.ts";
import "./mixins/dom_iterable_test.ts";
import "./mkdir_test.ts";
import "./net_test.ts";
import "./os_test.ts";
import "./platform_test.ts";
import "./process_test.ts";
import "./read_dir_test.ts";
import "./read_file_test.ts";
import "./read_link_test.ts";
import "./rename_test.ts";
import "./resources_test.ts";
import "./stat_test.ts";
import "./symlink_test.ts";
import "./text_encoding_test.ts";
import "./timers_test.ts";
import "./truncate_test.ts";
import "./url_test.ts";
import "./url_search_params_test.ts";
import "./write_file_test.ts";

import "../website/app_test.js";
