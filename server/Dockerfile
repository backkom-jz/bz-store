FROM rust:1.97-slim-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY server ./server
RUN cargo build --locked --release --package devstore-server
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/* && useradd --system --uid 10001 app
COPY --from=build /app/target/release/devstore-server /usr/local/bin/devstore-server
COPY --from=build /app/target/release/admin /usr/local/bin/admin
USER app
EXPOSE 8080
CMD ["devstore-server"]
