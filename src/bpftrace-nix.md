% Revamping bpftrace's CI with Nix

I've been rewriting the bpftrace CI on-and-off the past few months.  Now
that it's nearly done, I thought it would be nice to sum up some of the
changes.

### Previous state

The previous iteration of the bpftrace CI relied heavily on Github Actions and
docker. Specifically, we had three major workflows: CI, embedded, and code scanning.

The CI workflow was the main workhorse -- it built and tested bpftrace across
various combinations of LLVM version, distro base, and compiler suite (ie GCC
and clang).

The embedded workflow was fairly similar to the CI workflow except that it
built bpftrace in semi-static configurations. In other words, the bpftrace
binary under test would be statically linked with the exception of dynamic
linking against glibc and/or llvm/clang. If the tests passed, it would then
push the semi-static binary up to [quay.io][0] for end user consumption.

The code scanning job has seen various iterations over the years (from LGTM to
CodeQL) but the basic premise has been the same: to scan for known code smells
or bugs.

### Musings

I was very happy with Github Actions (GHA). The integration with Github is
obviously great and the free usage for open source projects is killer. So I had
very little desire to rip that out.

The docker bits were a different story. Over time, we grew somewhat of a
[maze][2] of [build scripts][3] that our infra as well as our Dockerfiles would
call into.  To make matters worse, almost all configuration was propagated
through environment variables, making adhoc analysis quite difficult. To top it
off, it was [not very user friendly][1] to run the CI configuration locally.

While I'll grant it would've been possible to refactor the docker setup so that
it was more user friendly, it still had the unavoidable drawback that binaries
built inside the container could not be easily run on the root host. This adds
friction to the development experience.

The final pain point we had was the embedded builds. The embedded builds were
awesome for our users -- a single binary that could run on almost any linux
system. That had nice properties like being able to `curl` it onto a host
you're debugging without worrying if the distro had packaged the latest version
of bpftrace yet. The drawback was that it was nearly impossible to maintain.
Dale Hamel did tremendously valuable work years ago adding it, but
unfortunately time has shown it was nearly impossible for the maintainers to
keep up. Static C++ binaries are notoriously difficult to build.  LLVM is
notoriously difficult to integrate and keep up with. Combine both together and
you have a serious problem.  Since most of the maintainers are volunteers, we
simply could not do it.

### Enter Nix

I had not planned on Nix fixing all these problems. Originally it was an
experiment for me to learn some new tech. But as I kept experimenting, I
realized Nix had a lot of awesome properties. I won't go into how I arrived at
the solutions -- it was an uninteresting combination of curiosity and long
walks. Rather, I'll lay out how it all fits together.

At the center of it all is the [bpftrace flake][4]. A [flake][5] is a Nix
feature that enables deterministic configuration and builds of software.
Inside our `flake.nix`, we define various bpftrace configurations. The main
variable we change is the LLVM version, as it's the most component bpftrace
needs to integrate with. We also define pinned versions of `bcc` and `libbpf`
as they are the other core bpftrace dependencies.

The flake immediately solves the developer experience problem. A new bpftrace
developer can simply install nix and run `nix build` to get a fully working
bpftrace binary. There are also other [convenient features][7] like developer
shells that allow for more incremental builds and easy debugging. The nice
thing is that all binaries built under Nix can run on the root host (not that
there was ever a non-root host involved).

Next is CI. To make running CI configurations easier locally, I wrote a
[dependency-free python script][8] that mirrored all the previously existing
CI features. The most salient bit of the python script (and the part that took
the longest to figure out) is [the code][9] that runs commands _inside_ the Nix
"environment":

```python
def shell(
    cmd: List[str],
    as_root: bool = False,
    cwd: Optional[Path] = None,
    env: Optional[Dict[str, str]] = None,
):
    """
    Runs the specified command in the proper nix development
    environment.

    Note that output is sent to our inherited stderr/stdout and
    that any errors immediately raise an exception.
    """
    [..]
```

With `ci.py` in place, all the workflow does is:

```yaml
    steps:
    - uses: actions/checkout@v2
    - uses: DeterminateSystems/nix-installer-action@v4
    - uses: DeterminateSystems/magic-nix-cache-action@v2
    - name: Load kernel modules
      # nf_tables and xfs are necessary for testing kernel modules BTF support
      run: |
        sudo modprobe nf_tables
        sudo modprobe xfs
    - name: Build and test
      env: ${{matrix.env}}
      run: ./.github/include/ci.py
```

And to run a basic CI configuration locally:

```shell
$ NIX_TARGET=.#bpftrace-llvm10  ./.github/include/ci.py
```

Full reproduction instructions are documented [here][10].

### Appimages

The embedded builds required a bit more lateral thinking. Semi-static builds
were clearly not tenable, so they were out. But we still need to give users a
replacement. At the end of the day, we can't screw our users just to make
maintenance easier.

Fortunately, I had learned about [AppImage][11]s recently. [This][12] is a
particularly well written motivation on the topic if you're not already
familiar. The make matters even better, there already exists Nix support for
appimage builds in the form of [nix-appimage][13].

Nix and appimages are a well-made match. Basically what a nix appimage does is
bundle a binary and all its runtime dependencies inside a squashfs image. It
then prepends to the image a statically linked entrypoint binary that handles
mounting the concatenated squashfs image through [squashfuse][14]. Once the
image is mounted, control is transferred to the `apprun` binary which bind
mounts the Nix store in a mount namespace at the places the final binary
expects and then runs the final binary.

To drive the elegance home, consider the [commit adding support to
bpftrace][15].  With support merged, the following is all you need to locally
build a fully static bpftrace binary:

```shell
$ nix build .#appimage
$ ldd ./result
        not a dynamic executable
$ sudo ./result -e 'BEGIN { print("static!"); exit() }'
Attaching 1 probe...
static!
```

With this, we can [delete][16] the embedded workflow and start shipping
appimages instead.

### CodeQL

Last but not least is the code scanning. The previous code scanning workflow
installed all the requisite build dependencies on the root host and then
ran the standard cmake build. That was fine, but it was a separate maintenance
path.

This [commit][17] brings Nix to the workflow. Note how we can leverage the
bpftrace flake while at the same time making things simpler.

### Conclusion

Like I mentioned earlier, while my Nix journey started off as an experiment, it
eventually proved to be useful beyond my wildest imagination. With new one
small file we now control developer experience, continuous integration, and
binary artifacts. All while greatly reducing maintenance burden.


[0]: https://quay.io/repository/iovisor/bpftrace?tab=tags&tag=latest
[1]: https://github.com/iovisor/bpftrace/blob/f56caa0b655d4ae12965ece8da04987a24708162/.github/workflows/ci.yml#L148-L191
[2]: https://github.com/iovisor/bpftrace/blob/f56caa0b655d4ae12965ece8da04987a24708162/docker/build.sh
[3]: https://github.com/iovisor/bpftrace/blob/f56caa0b655d4ae12965ece8da04987a24708162/build-libs.sh
[4]: https://github.com/iovisor/bpftrace/blob/e32fdb3af87ea9afc571e8df626e9172ef322c3c/flake.nix
[5]: https://nixos.wiki/wiki/Flakes
[6]: https://github.com/iovisor/bpftrace#nix
[7]: https://github.com/iovisor/bpftrace/blob/e32fdb3af87ea9afc571e8df626e9172ef322c3c/docs/nix.md
[8]: https://github.com/iovisor/bpftrace/blob/e32fdb3af87ea9afc571e8df626e9172ef322c3c/.github/include/ci.py
[9]: https://github.com/iovisor/bpftrace/blob/e32fdb3af87ea9afc571e8df626e9172ef322c3c/.github/include/ci.py#L89-L142
[10]: https://github.com/iovisor/bpftrace/blob/e32fdb3af87ea9afc571e8df626e9172ef322c3c/docs/developers.md#debugging-ci-failures
[11]: https://appimage.org/
[12]: https://blogs.gnome.org/tvb/2013/12/10/application-bundles-for-glade/
[13]: https://github.com/ralismark/nix-appimage
[14]: https://github.com/vasi/squashfuse
[15]: https://github.com/iovisor/bpftrace/commit/e32fdb3af87ea9afc571e8df626e9172ef322c3c
[16]: https://github.com/iovisor/bpftrace/pull/2742
[17]: https://github.com/iovisor/bpftrace/commit/529ef55e7fa64f23eb734f984a6d4867eabd071b
