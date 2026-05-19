FROM rust:1.95-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        binutils-mingw-w64-x86-64 \
        gcc-mingw-w64-x86-64-posix \
        g++-mingw-w64-x86-64-posix \
        mingw-w64-x86-64-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-pc-windows-gnu

ENV CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc-posix
ENV CARGO_TARGET_X86_64_PC_WINDOWS_GNU_AR=x86_64-w64-mingw32-ar
ENV CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc-posix
ENV CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++-posix

WORKDIR /app
COPY . .

RUN cargo build --release --target x86_64-pc-windows-gnu
RUN install -D target/x86_64-pc-windows-gnu/release/arp-scan-rs.exe /out/arp-scan-rs.exe

FROM debian:bookworm-slim AS artifact

COPY --from=builder /out/arp-scan-rs.exe /out/arp-scan-rs.exe
