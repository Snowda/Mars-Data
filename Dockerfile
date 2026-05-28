# syntax=docker/dockerfile:1

# ---- Build stage ----
# Pin to the toolchain declared in rust-toolchain.toml.
FROM rust:1.96-slim-bookworm AS builder

WORKDIR /build

# Cache dependency compilation: copy manifests first, build a stub, then the
# real sources. The dependency layer is reused until Cargo.toml changes.
COPY Cargo.toml ./
RUN mkdir src \
    && echo 'fn main() {}' > src/main.rs \
    && cargo build --release --bin marsdata \
    && rm -rf src

COPY src ./src

# Touch main.rs so cargo recompiles the crate (not the cached stub) and build.
RUN touch src/main.rs \
    && cargo build --release --bin marsdata \
    && strip target/release/marsdata

# ---- Runtime stage ----
# Distroless gives us a minimal image with CA certs (needed for the rustls
# HTTPS fetch of weather data) and no shell, shrinking the attack surface.
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

WORKDIR /app

COPY --from=builder /build/target/release/marsdata /usr/local/bin/marsdata
COPY config.toml ./config.toml

# Matches server.port in config.toml.
EXPOSE 3000

USER nonroot

ENTRYPOINT ["/usr/local/bin/marsdata"]
CMD ["serve"]
