FROM debian:bookworm-slim AS fetch

RUN apt-get update && apt-get install -y --no-install-recommends curl unzip ca-certificates && \
    rm -rf /var/lib/apt/lists/*
RUN curl -fsSL -o /tmp/kotlin-compiler.zip \
        https://github.com/JetBrains/kotlin/releases/download/v2.1.10/kotlin-compiler-2.1.10.zip && \
    unzip -q /tmp/kotlin-compiler.zip -d /opt && \
    rm /tmp/kotlin-compiler.zip

FROM eclipse-temurin:21-jre-alpine
COPY --from=fetch /opt/kotlinc /opt/kotlinc

RUN apk add --no-cache bash

RUN ln -s /opt/kotlinc/bin/kotlinc /usr/local/bin/kotlinc && \
    ln -s /opt/java/openjdk/bin/java /usr/local/bin/java
