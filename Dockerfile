FROM rust:1-bullseye AS native

ENV TOOLCHAIN_VERSION=armv6-rpi-linux-gnueabihf
ENV TOOLCHAIN_PATH=/opt/${TOOLCHAIN_VERSION}/bin
ENV PATH=${TOOLCHAIN_PATH}:${PATH}
ENV PKG_CONFIG_SYSROOT_DIR=/opt/${TOOLCHAIN_VERSION}/${TOOLCHAIN_VERSION}/sysroot/

WORKDIR /build

RUN true \
    && dpkg --add-architecture armhf \
    && apt-get update \
    && apt-get install --yes \
    libdbus-1-dev \
    libdbus-1-dev:armhf \
    liblzma-dev:armhf \
    libsystemd-dev:armhf \
    pkg-config \
    && rm --recursive --force /var/lib/apt/lists

RUN true \
    && wget --quiet --output-document=- \
    "https://github.com/tttapa/docker-arm-cross-toolchain/releases/latest/download/x-tools-${TOOLCHAIN_VERSION}.tar.xz" \
    | tar --extract --xz --directory=/opt \
    && mv /opt/x-tools/* /opt \
    && rmdir /opt/x-tools

RUN ln --symbolic "${TOOLCHAIN_PATH}/${TOOLCHAIN_VERSION}-gcc"  "${TOOLCHAIN_PATH}/arm-linux-gnueabihf-gcc"
RUN cp --recursive /lib/arm-linux-gnueabihf/* "${PKG_CONFIG_SYSROOT_DIR}/lib"
RUN cp --recursive /usr/lib/arm-linux-gnueabihf/* "${PKG_CONFIG_SYSROOT_DIR}/usr/lib"

RUN rustup target add arm-unknown-linux-gnueabihf

RUN mkdir --parents .cargo \
    && echo '[target.arm-unknown-linux-gnueabihf]' > .cargo/config.toml \
    && echo 'linker = "arm-linux-gnueabihf-gcc"' >> .cargo/config.toml

COPY src src
COPY Cargo.toml .
COPY Cargo.lock .

RUN true \
    && cargo fetch \
    #
    && cargo build --release \
    && cp /build/target/release/lywsd03mmc2influx /lywsd03mmc2influx.x86_64 \
    #
    && cargo build --release --target arm-unknown-linux-gnueabihf \
    && cp /build/target/arm-unknown-linux-gnueabihf/release/lywsd03mmc2influx /lywsd03mmc2influx.armhf \
    #
    && cargo clean
