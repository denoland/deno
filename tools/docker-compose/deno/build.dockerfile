FROM deno_prebuild

RUN ninja -j2 -C $BUILD_PATH mock_runtime_test deno_cc deno || echo "Exit with error"
RUN $BUILD_PATH/mock_runtime_test
RUN $BUILD_PATH/deno_cc foo bar
RUN $BUILD_PATH/deno meow
RUN ./tools/lint.sh
