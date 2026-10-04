# Imagen de Resguardo Server.
#   docker build -f packaging/servidor/Dockerfile -t resguardo-server .
#   docker run -d --name resguardo -p 8443:8443 -v resguardo-datos:/data resguardo-server
#   docker logs resguardo        # el código de primer arranque
FROM rust:1-bookworm AS compilar
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p resguardo-servidor && strip src-tauri/target/release/resguardo-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin resguardo \
 && mkdir -p /data && chown resguardo:resguardo /data && chmod 700 /data
COPY --from=compilar /src/src-tauri/target/release/resguardo-server /usr/local/bin/resguardo-server
COPY LICENSE NOTICE /usr/share/doc/resguardo-server/
USER resguardo
VOLUME ["/data"]
EXPOSE 8443
ENTRYPOINT ["/usr/local/bin/resguardo-server", "--datos", "/data", "--escuchar", "0.0.0.0:8443"]
