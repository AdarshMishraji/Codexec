FROM eclipse-temurin:21-jdk

RUN apt-get update && apt-get install -y --no-install-recommends unzip curl && \
    rm -rf /var/lib/apt/lists/*

RUN curl -fsSL -o /tmp/kotlin-compiler.zip \
        https://github.com/JetBrains/kotlin/releases/download/v2.1.10/kotlin-compiler-2.1.10.zip && \
    unzip -q /tmp/kotlin-compiler.zip -d /opt && \
    rm /tmp/kotlin-compiler.zip

    RUN ln -s /opt/kotlinc/bin/kotlinc /usr/local/bin/kotlinc && \
    ln -s /opt/java/openjdk/bin/java /usr/local/bin/java
