FROM deno_prebuild

RUN ninja -j2 -C $BUILD_PATH mock_runtime_test mock_main deno || echo "Exit with error"
RUN $BUILD_PATH/mock_runtime_test
RUN $BUILD_PATH/mock_main foo bar
RUN $BUILD_PATH/deno meow
RUN ./tools/lint.sh
