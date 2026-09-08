FROM dart:3.2.6

WORKDIR /opt/dart-template
RUN dart create -t console-simple codexec_deps --force && \
    cd codexec_deps && \
    dart pub add collection && \
    dart pub get

RUN ln -s /usr/lib/dart/bin/dart /usr/local/bin/dart
