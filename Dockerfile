FROM golang:1.10.2-stretch

RUN apk add --update go
RUN apk add --update git

RUN mkdir -p /go/src
ENV GOPATH /go
ENV PATH $PATH:$GOPATH/bin


RUN go get -u github.com/golang/protobuf/proto \
    github.com/golang/protobuf/protoc-gen-go \
    github.com/jteeuwen/go-bindata

RUN go get -d github.com/ry/v8worker2
RUN go get -u github.com/golang/protobuf/proto
RUN go get -u github.com/spf13/afero
RUN go get -u github.com/golang/protobuf/protoc-gen-go
RUN go get -u github.com/jteeuwen/go-bindata/...
RUN yarn

