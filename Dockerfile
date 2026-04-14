# syntax=docker/dockerfile:1.7

# ── Stage 1: Build ZeroClaw binary ───────────────────────────────────────────
FROM rust:1.93-slim AS builder

WORKDIR /app

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends pkg-config ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/robot-kit/Cargo.toml crates/robot-kit/Cargo.toml
RUN mkdir -p src benches crates/robot-kit/src && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > benches/agent_benchmarks.rs && \
    echo "pub fn placeholder() {}" > crates/robot-kit/src/lib.rs

RUN --mount=type=cache,id=zeroclaw-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=zeroclaw-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=zeroclaw-target,target=/app/target,sharing=locked \
    cargo build --release --locked

RUN rm -rf src benches crates/robot-kit/src
COPY src/ src/
COPY benches/ benches/
COPY crates/ crates/
COPY firmware/ firmware/

RUN --mount=type=cache,id=zeroclaw-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=zeroclaw-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=zeroclaw-target,target=/app/target,sharing=locked \
    cargo build --release --locked && \
    cp target/release/zeroclaw /app/zeroclaw && \
    strip /app/zeroclaw

# ── Stage 2: Runtime (Debian) ───────────────────────────────────────────────
FROM debian:trixie-slim AS runtime

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl bash git jq \
    python3 python3-pip python3-venv \
    nodejs npm \
    docker-cli docker-compose \
    && rm -rf /var/lib/apt/lists/*

# Python virtual environment baked into image (not mounted volume).
RUN python3 -m venv /opt/venv && \
    /opt/venv/bin/pip install --no-cache-dir --upgrade pip setuptools wheel && \
    /opt/venv/bin/pip install --no-cache-dir \
      pandas numpy scipy scikit-learn requests pyyaml python-dotenv plotly

# ByteRover CLI
RUN npm install -g @campfirein/byterover-cli

RUN mkdir -p /zeroclaw-data/.zeroclaw /zeroclaw-data/workspace /opt/zeroclaw

COPY --from=builder /app/zeroclaw /usr/local/bin/zeroclaw

# IMPORTANT FIX: copy production config, not dev/config.template.toml.
COPY docker-config/config.toml /opt/zeroclaw/default-config.toml
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

ENV ZEROCLAW_WORKSPACE=/zeroclaw-data/workspace
ENV HOME=/zeroclaw-data
ENV PATH="/opt/venv/bin:${PATH}"
ENV ZEROCLAW_GATEWAY_PORT=3000

WORKDIR /zeroclaw-data
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["daemon"]

# ── Stage 3: Optional distroless release binary image ───────────────────────
FROM gcr.io/distroless/cc-debian13:nonroot AS release
COPY --from=builder /app/zeroclaw /usr/local/bin/zeroclaw
WORKDIR /zeroclaw-data
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/zeroclaw"]
CMD ["gateway"]
