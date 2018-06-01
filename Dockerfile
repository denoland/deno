FROM golang:1.10-stretch

RUN apt-get update && apt-get install -y \
    unzip \
    # Deps for v8worker2 build
    xz-utils \
    lbzip2 \
    libglib2.0

RUN curl -sL https://deb.nodesource.com/setup_8.x | bash - \
    && apt-get update && apt-get install -y nodejs \
    && npm install -g yarn

RUN wget https://github.com/google/protobuf/releases/download/v3.1.0/protoc-3.1.0-linux-x86_64.zip \
    && unzip protoc-3.1.0-linux-x86_64.zip \
    && mv bin/protoc /usr/local/bin \
    && rm -rf include \
    && rm readme.txt \
    && rm protoc-3.1.0-linux-x86_64.zip

RUN go get -u github.com/golang/protobuf/protoc-gen-go
RUN go get -u github.com/jteeuwen/go-bindata/...

# Pulling submodules manually, errors abound with go get
# See: https://github.com/ry/deno/issues/92
RUN mkdir -p $GOPATH/src/github.com/ry/v8worker2 
RUN cd $GOPATH/src/github.com/ry/v8worker2 \
    && git clone https://github.com/ry/v8worker2.git . \
    && rm -rf v8 \
    && git clone https://github.com/v8/v8.git && cd v8 \
    && git checkout fe12316ec4b4a101923e395791ca55442e62f4cc \
    && cd .. \
    && rm -rf depot_tools \
    && git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git \
    && git submodule update --init --recursive

# v8worker2 build wants a valid git config
RUN git config --global user.email "you@example.com"
RUN git config --global user.name "Your Name"

RUN cd $GOPATH/src/github.com/ry/v8worker2 && python -u ./build.py

# Will not exit cleanly before make populates proto structs
RUN go get -u github.com/ry/deno/... || true

WORKDIR $GOPATH/src/github.com/ry/deno
RUN make

CMD ./deno
