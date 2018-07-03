FROM deno_base

# prebuild
ENV BUILD_PATH=/out/Debug
RUN git clone https://github.com/ry/deno.git 
#RUN rm -f /deno/tools/clang
#RUN rm -f /deno/tools/protoc_wrapper
#RUN rm -f /deno/build 
#RUN rm -f /deno/build_overrides
#RUN rm -f /deno/buildtools
#RUN rm -f /deno/testing
RUN cd /deno/third_party && gclient sync --no-history
RUN cd /deno/js && yarn install
RUN cd /deno/ && gn gen $BUILD_PATH --args='is_debug=false use_allocator="none" cc_wrapper="ccache" use_custom_libcxx=false use_sysroot=false'
RUN cd /deno/ && gn args $BUILD_PATH --list
RUN cd /deno/ && gn desc $BUILD_PATH :deno
