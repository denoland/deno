FROM deno_prebuild
RUN cd /deno/src && ninja -j2 -C out/Debug/ mock_runtime_test 2>&1 | tee /build.log || echo "Exit with error"
RUN cd /deno/src && ninja -j2 -C out/Debug/ mock_main 2>&1 | tee -a /build.log || echo "Exit with error" 
RUN cd /deno/src && ninja -j2 -C out/Debug/ deno 2>&1 | tee -a /build.log || echo "Exit with error" 
