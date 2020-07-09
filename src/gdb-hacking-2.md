% GDB hacking part 2

One of the issues I've had with GDB is reliably building the code. The project
apparently checks in automake-1.15 artifacts and my host has automake-1.16, so
that means any code changes that trigger automake regeneration causes failures.
Arch linux doesn't package auotmake-1.15 like some other distros so I've had to
get clever.

My solution was to wrap the build in a docker container to effectively document
the build. The thinking goes that I only need to get the build to work once and
then it'll work forever.

The way it works is I have an `x.py`:

```bash
$ ./x.py help
usage: x [-h] [-s SOURCE_DIR] [-b BUILD_DIR] {conf,build,run,shell,test,help} ...

positional arguments:
  {conf,build,run,shell,test,help}
                        subcommands
    conf                configure and build gdb container image
    build               build gdb
    run                 run gdb
    shell               open shell
    test                run `make check tests`
    help                print help

optional arguments:
  -h, --help            show this help message and exit
  -s SOURCE_DIR, --source-dir SOURCE_DIR
                        source code directory (default: ~/dev/gdb)
  -b BUILD_DIR, --build-dir BUILD_DIR
                        build directory (default: /tmp/gdb-build)

```

Running something like `./x.py build` effectively expands to:

```bash
podman run                         \
  -v=/home/daniel/dev/gdb:/gdb/src \
  -v=/tmp/gdb-build:/gdb/build     \
  localhost/gdb-builder            \
  make -C build -j4
```

To those not familiar with `docker`, the command bind mounts the source and
build directories to directories inside the container and then it runs `make -C
build -j4`. Note I've had to use `podman`/`crun` instead of the usual
`docker`/`runc` because I run `cgroup2` on my host (I do some cgroup2 related
development) and docker doesn't support cgroup2 yet.

The `gdb-builder` image is built by the (simplified for post) `Containerfile`:

```docker
FROM ubuntu

RUN apt-get update
RUN apt-get install -y \
  automake-1.15 \
  bash \
  bison \
  build-essential \
  curl \
  dejagnu \
  flex \
  g++ \
  libncurses-dev \
  libreadline-dev \
  texinfo \
  xsltproc \
  zlib1g-dev

WORKDIR /gdb

COPY scripts/configure.sh configure.sh
RUN chmod 755 configure.sh
```

### Final thoughts

I've never really used docker before and I thought this infrastructure would be
a good way to play around. I'm fairly happy I did this because docker ended up
being a fairly ergonomic way to document the build. Every time I need something
new, it's fairly easy to add another subcommand to save the command line
invocation.  The command line invocation is fairly certain to work b/c it's all
done inside a reproducible container.

### Code

The full script repository is [available
here](https://github.com/danobi/gdb-scripts).
