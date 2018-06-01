FROM ubuntu:18.04

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    g++ \
    gcc \
    libc6-dev \
    make \
    pkg-config \
    curl \
    ca-certificates \
    gnupg \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/*

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    apt-transport-https \
    unzip \
    bsdtar \
    ccache \
    build-essential \
    python \
    git \
    libperl-dev \
    libgtk2.0-dev \
    golang golang-src \
    clang clang-format-5.0 \
  && curl -sS https://dl.yarnpkg.com/debian/pubkey.gpg | apt-key add - \
  && echo "deb https://dl.yarnpkg.com/debian/ stable main" | tee /etc/apt/sources.list.d/yarn.list \
  && apt-get update \
  && apt-get install -y \
    yarn \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/*

ENV HOME               /
ENV V8WORKER2_OUT_PATH /v8worker2_out
ENV PROTOBUF_ROOT      /protobuf
ENV GOPATH             /go

ENV PATH               $PROTOBUF_ROOT/bin:${GOPATH}/bin:/usr/lib/llvm-5.0/bin:${PATH}

RUN mkdir -p $PROTOBUF_ROOT \
  && cd $PROTOBUF_ROOT \
  && curl -sSL https://github.com/google/protobuf/releases/download/v3.1.0/protoc-3.1.0-linux-x86_64.zip | bsdtar -xvf- \
  && chmod +x $PROTOBUF_ROOT/bin/protoc

RUN  go get -u github.com/golang/protobuf/proto \
  && go get -u github.com/golang/protobuf/protoc-gen-go \
  && go get -u github.com/spf13/afero \
  && go get -u github.com/jteeuwen/go-bindata/...

RUN go get -u github.com/ry/v8worker2 || true \
  && ccache -s \
  && cd $GOPATH/src/github.com/ry/v8worker2 \
  && ./build.py --use_ccache --out_path $V8WORKER2_OUT_PATH \
  && rm -R /.vpython-root /.ccache \
  && cd ${GOPATH}/src \
  && for file in `find . -name .git`;do rm -rf $file;done

# change permissions to allow access to any user
RUN chmod -R a+rX /v8worker2_out

ENV PKG_CONFIG_PATH $GOPATH/src/github.com/ry/v8worker2

WORKDIR $GOPATH/src/github.com/ry/deno

ENV YARN_WRAP_OUTPUT true

CMD ["make"]
