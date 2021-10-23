# How to cross-compile for the Raspberry Pi

<small>Ibiyemi Abiodun, October 2021</small>

In order to cross-compile the plane system so that it can run on a Raspberry Pi
running Raspbian, you basically have to DIY everything. This is because by
default, Rust generates executables that are dynamically linked, meaning that
they don't contain everything they need in order to run, and instead they link
to a library called `glibc` at runtime. At the time of writing, Raspbian uses a
very old version of `glibc` (version 2.28, from 2018). However, the version of
`glibc` that your Rust executable will link against is the version used in your
cross-compilation toolchain, and if this is higher than the version included in
your Raspberry Pi's operating system, then your program will fail to run. Modern
versions of cross compilers are likely to include the most recent version of
`glibc` (version 3.34 at the time of writing).

For this reason, you must either:
- Install a recent version of Debian or Ubuntu on your Raspberry Pi instead of
  using Raspbian
- Compile a cross compiler from scratch with `glibc` 2.28

## If you chose the first option

Then things should be easy! There should be packages available for your
operating system that allow you to cross compile to the
`armv7-linux-gnueabihf`/`arm-linux-gnueabihf` targets (Pi 2 / 3) or
`aarch64-linux-gnu` target (Pi 4). 

## If you chose the second option

I found [this excellent
tutorial](https://preshing.com/20141119/how-to-build-a-gcc-cross-compiler/) for
creating a cross compiler from scratch. Here's how it goes, with updated versions of everything:

### Prerequisite information

Run the following commands on your Raspberry Pi:

- `uname -r`: this will tell you the version of the Linux kernel that its OS uses
- `lscpu`: the important information is the CPU architecture (which should be `armv6l`, `armv7l`, or `armv8l`/`aarch64`) and the flags (flags should contain `vfp`, indicating that the processor has a floating-point co-processor)
- `ldd --version`: this will tell you which version of `glibc` your operating system is using
- `cat /etc/os-release`: this will tell you which distro of Linux is running and what version it is

### Install the basic utilities for compilation

Make sure that `g++`, `gawk`, and `make` are installed on your system. You can
probably install them using your system package manager.

### Get the dependencies of GCC 

These are `mpfr`, `gmp`, `mpc`, `isl`, and `cloog`. You can probably install
them using your system package manager, but if not, then you can download them
from http://ftpmirror.gnu.org/ and
https://gcc.gnu.org/pub/gcc/infrastructure/. You can unzip the `.tar.xz` files
using `tar xf /path/to/file`.

### Get GCC

This can be downloaded from https://gcc.gnu.org/pub/gcc/. I used version
`8.5.0`, because it was released around the same time as `glibc` 2.28. 

> I tried using the most recent `11.3.0`, and it did not work because GCC 11
> flags more things as errors than GCC 8, and so only newer versions of `glibc`
> with these errors fixed will compile using GCC 11. **You should use the most
> recent version of everything else mentioned in this document**, unless it
> doesn't work, in which case, use an older version 😜

If you downloaded GCC's dependencies as `.tar.xz` files instead of installing
them using the package manager, you should now symlink them into the GCC source folder:

```bash
cd gcc-8.5.0
ln -s ../mpfr-x.x.x mpfr
ln -s ../gmp-x.x.x gmp
ln -s ../mpc-x.x.x mpc
ln -s ../isl-x.x.x isl
ln -s ../cloog-x.x.x cloog
cd ..
```
### Create your toolchain directory

We're going to install everything we need to cross compile (the
cross-compilation toolchain) into `/opt/cross`. Create this folder and make
yourself the owner.

```bash
sudo mkdir -p /opt/cross
sudo chown jeff /opt/cross
```

Now add `/opt/cross/bin` to your PATH. **This is very important, and later steps
will fail if you do not do this.** Later steps will depend on the executables
you build in earlier steps, and these executables are being placed in
`/opt/cross/bin`.

```bash
export PATH=/opt/cross/bin:$PATH
```

### Compile binutils

These are the cross-assembler, cross-linker, and other tools.

> In some `make` commands, you may seen an option `-j`. `-j` is the number of
> threads that `make` uses, so if you have more than 4 cores on your PC, you can
> increase this number to get faster builds. I use `-j8` on my device which has
> 12 cores.

```bash
mkdir build-binutils
cd build-binutils
../binutils-x.xx/configure --prefix=/opt/cross --target=armv7l-linux-gnueabihf
make -j4
make install
cd ..
```

### Add Linux kernel headers

These are needed so that the programs we compile using this toolchain will be able to make system calls into the Linux kernel. They can be downloaded from https://www.kernel.org/pub/linux/kernel/. The version you choose doesn't matter too much, but try to choose one that is close to the Linux kernel version on your Raspberry Pi.

After downloading, install them into your cross-compiler directory:

```bash
cd linux-x.xx.x
make ARCH=arm INSTALL_HDR_PATH=/opt/cross/armv7l-linux-gnueabihf headers_install
cd ..
```

> Change `ARCH=arm` to `ARCH=arm64` if using a 64-bit ARM target such as `aarch64`

### Build the C compiler

Confusingly, the GCC compiler suite and `glibc` depend on each other. So the
first thing you have to do is build only the C compiler from GCC.

```bash
mkdir -p build-gcc
cd build-gcc
../gcc-8.5.0/configure --prefix=/opt/cross --target=armv7l-linux-gnueabihf --enable-languages=c
make -j4 all-gcc
make install-gcc
cd ..
```

### Build the C Library Headers and Startup Files

Then, use that basic C compiler to generate stubs and header files for `glibc`,
which will allow us to do the next step.

```bash
mkdir -p build-glibc
cd build-glibc
../glibc-2.28/configure --prefix=/opt/cross/armv7l-linux-gnueabihf --build=$MACHTYPE --host=armv7l-linux-gnueabihf --target=armv7l-linux-gnueabihf --with-headers=/opt/cross/armv7l-linux-gnueabihf/include libc_cv_forced_unwind=yes
make install-bootstrap-headers=yes install-headers
make -j4 csu/subdir_lib
install csu/crt1.o csu/crti.o csu/crtn.o /opt/cross/armv7l-linux-gnueabihf/lib
armv7l-linux-gnueabihf-gcc -nostdlib -nostartfiles -shared -x c /dev/null -o /opt/cross/armv7l-linux-gnueabihf/lib/libc.so
touch /opt/cross/armv7l-linux-gnueabihf/include/gnu/stubs.h
cd ..
```

### Build the Compiler Support Library

The compiler support library requires these stubs and header files, and will
allow us to finish compiling `glibc`.

```bash
cd build-gcc
make -j4 all-target-libgcc
make install-target-libgcc
cd ..
```

### Build the standard C library

Finally, we are building `glibc`.

```bash
cd build-glibc
make -j4
make install
cd ..
```

And we're done! We are using this cross-compilation toolchain to link our Rust
binaries, which is why we need the C standard library to link against, and the
cross-linker and cross-assembler to link our Rust binary to it.

We are not going to be using this cross-compiler to compile any C++ code, so the
C++ standard library is not needed. Therefore, I have omitted that step from the
tutorial.
