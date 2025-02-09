% Nix dependency tree

This tip is so useful to me but simultaneously so hard to find that I'm going
to dedicate an entire entry to it.

How to find all the dependencies a flake output pulls in:

```shell
$ nix path-info .#bpftrace-llvm18
[..]
this derivation will be built:
  /nix/store/ada4fcj69rqqqxvdzg3f0gvwm6pmzrch-bpftrace.drv

$ nix-store --query --tree /nix/store/ada4fcj69rqqqxvdzg3f0gvwm6pmzrch-bpftrace.drv
/nix/store/ada4fcj69rqqqxvdzg3f0gvwm6pmzrch-bpftrace.drv
├───/nix/store/v6x3cs394jgqfbi0a42pam708flxaphh-default-builder.sh
├───/nix/store/s63zivn27i8qv5cqiy8r5hf48r323qwa-bash-5.2p37.drv
│   ├───/nix/store/05q48dcd4lgk4vh7wyk330gr2fr082i2-bootstrap-tools.drv
│   │   ├───/nix/store/0m4y3j4pnivlhhpr5yqdvlly86p93fwc-busybox.drv
[...]
```

There's a lot of output, but the full dependency tree should be there. It'll
help you answer questions like "why is XXX package being pulled in?".

A secondary tip is that if a dependency is directly attached to your root, it
means some kind of Nix magic is pulling it in. For example, a build rule could
be directly referencing a package.
