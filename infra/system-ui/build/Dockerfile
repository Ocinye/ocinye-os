# Build environment for the native system surfaces of Ocinye OS (ADR-0031): the
# compositor and the Tauri windows. They link the graphics and web view
# libraries of the base system, so they are built on that base and for the
# target architecture — never cross-compiled from a developer's machine.
#
# The base is the same Ubuntu the images are made of. Nothing here ships: only
# the binaries built inside it do.
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    PATH=/opt/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential ca-certificates curl pkg-config \
      # compositor: session, input, KMS, buffers, keyboard, Wayland
      libseat-dev libinput-dev libudev-dev libdrm-dev libgbm-dev \
      libxkbcommon-dev libwayland-dev libegl-dev libgles-dev libpixman-1-dev \
      # Tauri windows: GTK and the system web view
      libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
      librsvg2-dev libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# The toolchain the workspace pins (rust-version in Cargo.toml).
ARG RUST_VERSION=1.98.0
RUN curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o /tmp/rustup-init.sh \
    && sh /tmp/rustup-init.sh -y --profile minimal --default-toolchain "${RUST_VERSION}" --no-modify-path \
    && rm /tmp/rustup-init.sh \
    && chmod -R a+rX /opt/rustup /opt/cargo

WORKDIR /src
