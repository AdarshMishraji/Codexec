FROM eclipse-temurin:25-jdk

RUN ln -s /opt/java/openjdk/bin/java /usr/local/bin/java && \
    ln -s /opt/java/openjdk/bin/javac /usr/local/bin/javac

RUN mkdir -p /opt/codexec/src/codexec/util /opt/codexec/lib && \
    cat > /opt/codexec/src/codexec/util/Pair.java <<'EOF'
package codexec.util;

import java.util.Objects;

/** Drop-in substitute for javafx.util.Pair (not available in a headless JDK). */
public class Pair<K, V> {
    private final K key;
    private final V value;

    public Pair(K key, V value) {
        this.key = key;
        this.value = value;
    }

    public K getKey() { return key; }
    public V getValue() { return value; }

    @Override
    public String toString() { return key + "=" + value; }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Pair<?, ?> p)) return false;
        return Objects.equals(key, p.key) && Objects.equals(value, p.value);
    }

    @Override
    public int hashCode() { return Objects.hash(key, value); }
}
EOF
RUN javac -d /opt/codexec/classes /opt/codexec/src/codexec/util/Pair.java && \
    jar --create --file /opt/codexec/lib/pair.jar -C /opt/codexec/classes .
