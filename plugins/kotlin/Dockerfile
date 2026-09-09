FROM debian:bookworm-slim AS fetch

RUN apt-get update && apt-get install -y --no-install-recommends curl unzip ca-certificates && \
    rm -rf /var/lib/apt/lists/*
RUN curl -fsSL -o /tmp/kotlin-compiler.zip \
        https://github.com/JetBrains/kotlin/releases/download/v2.1.10/kotlin-compiler-2.1.10.zip && \
    unzip -q /tmp/kotlin-compiler.zip -d /opt && \
    rm /tmp/kotlin-compiler.zip

# kotlinc is itself a JVM program with its own bundled compiler classes -
# it needs a `java` to run on, but (for a single .kt file, no mixed Java
# sources) never shells out to `javac`, `jshell`, `jdb`, or any of the
# other JDK-only tooling. A JRE is all the final image needs; curl/unzip
# (needed only to fetch/unpack the compiler above) never make it into the
# final image at all.
FROM eclipse-temurin:21-jre-alpine
COPY --from=fetch /opt/kotlinc /opt/kotlinc

# kotlinc's own launcher script (/opt/kotlinc/bin/kotlinc) has a `bash`
# shebang, not `sh` - Alpine's default shell is BusyBox ash, which isn't
# enough (confirmed: fails with "env: 'bash': No such file or directory"
# with no bash installed). bash itself is tiny next to the JDK/JRE size
# difference this base otherwise saves.
RUN apk add --no-cache bash

RUN ln -s /opt/kotlinc/bin/kotlinc /usr/local/bin/kotlinc && \
    ln -s /opt/java/openjdk/bin/java /usr/local/bin/java
