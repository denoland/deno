// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

static TEST_REGISTRY_URL: &str = "http://127.0.0.1:4250";

pub fn env_vars_for_registry() -> Vec<(String, String)> {
  vec![(
    "DENO_REGISTRY_URL".to_string(),
    TEST_REGISTRY_URL.to_string(),
  )]
}

itest!(no_token {
  args: "do-not-use-publish publish/missing_deno_json",
  output: "publish/no_token.out",
  exit_code: 1,
});

itest!(missing_deno_json {
  args:
    "do-not-use-publish --token 'sadfasdf' $TESTDATA/publish/missing_deno_json",
  output: "publish/missing_deno_json.out",
  exit_code: 1,
  temp_cwd: true,
});

itest!(successful {
  args: "do-not-use-publish --token 'sadfasdf' $TESTDATA/publish/successful",
  output: "publish/successful.out",
  envs: env_vars_for_registry(),
  http_server: true,
  temp_cwd: true,
});
