FROM debian:buster AS cc

RUN useradd -m ccuser
RUN mkdir -p /opt/cross
RUN chown -R ccuser /opt/cross

ENV PATH="/opt/cross/bin:${PATH}"

# install dependencies of GCC and other basic utils
RUN apt-get update && apt-get install -y --no-install-recommends \
	libmpfr-dev libmpc-dev libgmp-dev \
	autotools-dev autoconf file rsync flex bison binutils gawk gcc g++ make \
	ca-certificates curl xz-utils lbzip2 bzip2 \
	&& rm -rf /var/lib/apt/lists/*


USER ccuser
WORKDIR "/tmp"

ARG GCC_VERSION

# download and extract GCC
RUN echo "downloading gcc version ${GCC_VERSION}" && curl -f -O https://gcc.gnu.org/pub/gcc/releases/gcc-${GCC_VERSION}/gcc-${GCC_VERSION}.tar.xz
RUN tar -xf gcc-${GCC_VERSION}.tar.xz

RUN echo "downloading isl 0.18" && curl -f -O https://gcc.gnu.org/pub/gcc/infrastructure/isl-0.18.tar.bz2
RUN tar -xf isl-0.18.tar.bz2

RUN echo "downloading cloog1 0.18.1" && curl -f -O https://gcc.gnu.org/pub/gcc/infrastructure/cloog-0.18.1.tar.gz
RUN tar -xf cloog-0.18.1.tar.gz

RUN ln -s ../isl-0.18 gcc-8.5.0/isl
RUN ln -s ../cloog-0.18.1 gcc-8.5.0/cloog

ARG BINUTILS_VERSION

# download and extract binutils
RUN echo "downloading binutils version ${BINUTILS_VERSION}" && curl -f -O https://ftp.gnu.org/gnu/binutils/binutils-${BINUTILS_VERSION}.tar.xz
RUN tar xf binutils-${BINUTILS_VERSION}.tar.xz

ARG TARGET_GCC

# build and install binutils
RUN mkdir -p /tmp/build-binutils
WORKDIR "/tmp/build-binutils"
RUN echo "configuring binutils version ${BINUTILS_VERSION} with target ${TARGET_GCC}" && ../binutils-${BINUTILS_VERSION}/configure --prefix=/opt/cross --target=${TARGET_GCC} --enable-gold --disable-nls
RUN make -j8 && make install

ARG LINUX_SERIES
ARG LINUX_VERSION
ARG TARGET_LINUX

# download and extract linux headers
WORKDIR "/tmp"
RUN echo "downloading linux version ${LINUX_VERSION}" && curl -f -O https://mirrors.edge.kernel.org/pub/linux/kernel/v${LINUX_SERIES}/linux-${LINUX_VERSION}.tar.xz
RUN tar xf linux-${LINUX_VERSION}.tar.xz

# install linux headers
WORKDIR "/tmp/linux-${LINUX_VERSION}"
RUN make ARCH=${TARGET_LINUX} INSTALL_HDR_PATH=/opt/cross/${TARGET_GCC} headers_install

ARG GCC_MULTILIBS

# build and install gcc
RUN mkdir -p /tmp/build-gcc
WORKDIR "/tmp/build-gcc"
RUN ../gcc-${GCC_VERSION}/configure --prefix=/opt/cross --target=${TARGET_GCC} --enable-languages=c,c++ --with-multilib-list=${GCC_MULTILIBS} --disable-bootstrap  --disable-nls
RUN make all-gcc
RUN make install-gcc

ARG GLIBC_VERSION

# download and extract glibc
WORKDIR "/tmp"
RUN curl -O https://ftp.gnu.org/gnu/glibc/glibc-${GLIBC_VERSION}.tar.xz
RUN tar xf glibc-${GLIBC_VERSION}.tar.xz

# build glibc stage 1
RUN mkdir -p /tmp/build-glibc
WORKDIR "/tmp/build-glibc"
RUN ../glibc-${GLIBC_VERSION}/configure --prefix=/opt/cross/${TARGET_GCC} --build=$MACHTYPE --host=${TARGET_GCC} --target=${TARGET_GCC} --with-headers=/opt/cross/${TARGET_GCC}/include libc_cv_forced_unwind=yes
RUN make install-bootstrap-headers=yes install-headers
RUN make csu/subdir_lib
RUN install csu/crt1.o csu/crti.o csu/crtn.o /opt/cross/${TARGET_GCC}/lib
RUN ${TARGET_GCC}-gcc -nostdlib -nostartfiles -shared -x c /dev/null -o /opt/cross/${TARGET_GCC}/lib/libc.so
RUN touch /opt/cross/${TARGET_GCC}/include/gnu/stubs.h
RUN touch /opt/cross/${TARGET_GCC}/include/gnu/stubs-hard.h
RUN touch /opt/cross/${TARGET_GCC}/include/gnu/stubs-soft.h

# build libgcc
WORKDIR "/tmp/build-gcc"
RUN make all-target-libgcc
RUN make install-target-libgcc

# build glibc stage 2
WORKDIR "/tmp/build-glibc"
RUN make -j8 && make install

USER root

# install dependencies of GCC and plane system
RUN dpkg --add-architecture armhf
RUN apt-get update && apt-get install -y --no-install-recommends \
	libusb-1.0-0-dev:armhf libudev-dev:armhf libgstreamer1.0-dev:armhf \
	&& rm -rf /var/lib/apt/lists/*

USER ccuser
WORKDIR "/home/ccuser"

# install rust toolchain
ARG TARGET_RUST
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/home/ccuser/.cargo/bin:${PATH}"
RUN rustup target add ${TARGET_RUST}

# add rust config
RUN mkdir -p .cargo && printf "[target.${TARGET_RUST}]\nlinker = \"/opt/cross/bin/${TARGET_GCC}-gcc\"\n" > .cargo/config.toml

# add env vars
ARG CFLAGS
ENV CFLAGS=${CFLAGS}
ENV CC=${TARGET_GCC}-gcc

# run
VOLUME /app
WORKDIR "/app"
ENV TARGET_RUST=${TARGET_RUST}
CMD cargo build --target ${TARGET_RUST}
