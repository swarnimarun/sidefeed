FROM rust:1.85-bookworm AS build
WORKDIR /app
COPY Cargo.toml ./
COPY migrations ./migrations
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home sidefeed
COPY --from=build /app/target/release/sidefeed /usr/local/bin/sidefeed
USER sidefeed
WORKDIR /home/sidefeed
ENV SIDEFEED_LISTEN=0.0.0.0:8080 \
    SIDEFEED_DATABASE_URL=sqlite:///home/sidefeed/sidefeed.db?mode=rwc
EXPOSE 8080
VOLUME ["/home/sidefeed"]
ENTRYPOINT ["sidefeed"]
