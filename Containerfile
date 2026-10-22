# Mars-Data weather API image. Build and run with Podman:
#   podman build -f Containerfile -t localhost/mars-data:dev .
#   podman run --rm -p 127.0.0.1:3000:3000 -v ./my-config.toml:/app/config.toml:ro localhost/mars-data:dev
# The baked config.toml binds 127.0.0.1, which a published port cannot reach: mount one with host = "0.0.0.0".

# ---- Build stage ----
# Pin to the toolchain declared in rust-toolchain.toml. Fully qualified: Podman has no default registry.
FROM docker.io/library/rust:1.99.1-slim-bookworm AS builder

WORKDIR /build

# Cache dependency compilation: copy the manifest first, build a stub, then the real sources.
# The dependency layer is reused until Cargo.toml changes.
COPY Cargo.toml ./
# fuzz/ is a workspace member for local fuzzing only; drop it so the image builds just the server.
RUN sed -i -e '/^members = /d' -e '/^default-members = /d' Cargo.toml \
    && mkdir src \
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
