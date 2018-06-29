FROM deno_dependency
ENV BUILD_PATH=/out/Default
RUN echo $BUILD_PATH
ENV DEPOT_TOOLS_PATH=/depot_tools
ENV RUST_PATH=/root/.cargo/bin
ENV PATH="${PATH}:${RUST_PATH}:${DEPOT_TOOLS_PATH}"
RUN echo $PATH
COPY ./src /src
RUN cd /src && gclient sync --no-history
RUN cd /src/js && yarn install
RUN cd /src && gn gen out/Debug --args='is_debug=false use_allocator="none" cc_wrapper="ccache" use_custom_libcxx=false use_sysroot=false'
RUN cd /src && gn args out/Debug/ --list
RUN cd /src && gn desc out/Debug/ :deno
