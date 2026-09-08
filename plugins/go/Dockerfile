FROM golang:1.23

RUN ln -s /usr/local/go/bin/go /usr/local/bin/go

ENV GOMODCACHE=/opt/go/pkg/mod
WORKDIR /opt/go-template
RUN go mod init codexec-submission && \
    go get github.com/emirpasic/gods@v1.18.1 && \
    go get github.com/emirpasic/gods/v2@v2.0.0-alpha && \
    go mod download
