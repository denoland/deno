FROM deno_prebuild
RUN cd /deno && ninja -j2 -C $BUILD_PATH mock_runtime_test 2>&1 | tee /build.log || echo "Exit with error"
RUN cd /deno && ninja -j2 -C $BUILD_PATH deno 2>&1 | tee -a /build.log || echo "Exit with error" 
