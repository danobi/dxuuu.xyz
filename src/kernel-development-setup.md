% Creating a basic Linux kernel development setup

Linux kernel development is something I've been wanting to get into for a long time now. I've tried and
failed to get started over the years, but I finally think I'm in a place where I can do some real kernel
work. But before we can dive into kernel development, we have to have a working development setup.

This document will be a rough outline into

1. Compiling the kernel
2. Setting up a initramfs for the kernel to boot into
3. Booting into our setup

### Compiling the kernel

Compiling the kernel is a relatively straightforward task. First, do as you would any other open source
project and fork Linus' [kernel repo][0] on Github. Alternatively, you could clone his kernel.org repo, but
it doesn't really matter.

`cd` into your cloned repo and run `make defconfig; make x86_64_defconfig; make kvmconfig`. This will generate
a pretty good set of default configs (located in `.config`).

* `make defconfig` generates a default `.config`
* `make x86_64_defconfig` makes some x86 specific changes to the new `.config`
* `make kvmconfig` enables some kvm specific kernel options that we will later take advantage of

Finally run `make` to compile the kernel. Hint: use `make -jN`, where `N` is the number of logical cores
you have on your CPU for faster compilation.

### Setting up initramfs

[initramfs][1] is the initial memory only filesystem the Linux kernel uses during boot. It is necessary
to solve some of the boot time chicken-or-egg type problems that may arise. Read more from the link if curious.
If we were to try and boot into a bare kernel, we would get a kernel panic about not being able to mount
a rootfs. This is expected because of the previous reasons. Thus, we need to create our own initramfs.

There are a number of ways to create an initial ramdisk. We are going to roll our own busybox ramdisk.
Although it's not the easiest way, it's rather transparent and somewhat easy to understand.

First, clone the [busybox repo][2]. Then, `cd` into the directory and run `make defconfig`. Sounds familiar,
right? Next, run `make menuconfig` and make sure the static binary option is ticked. You may need to dig
around in that interface, but it's there. Finding it is left as an exercise to the reader. Finally, run
`make; make install`. This will put all the symlinked busybox "binaries" into `_install` of the current
directory.

As for the initramfs, create a folder for holding all the intermediate files. For this guide, we'll assume
it's in `~/dev/initramfs`.

    mkdir -pv ~/dev/initramfs/x86-busybox
    cd !$
    mkdir -pv {bin,sbin,etc,proc,sys,usr/{bin,sbin}}
    cp -av ~/dev/busybox/_install/* .

What this does is create a basic filesystem layout for our custom Linux kernel to boot into. Obviously
some utilities that rely on a more complicated filesystem layout won't work. Finding out which ones is
another exercise left to the reader.

Now we need an init script. All Linux systems need an inital process to startup all the userspace fun,
and this is no different. Plop this simple init script into `~/dev/initramfs`:

    #!/bin/sh

    mount -t proc none /proc
    mount -t sysfs none /sys

    echo -e "\nBoot took $(cut -d' ' -f1 /proc/uptime) seconds\n"

    exec /bin/sh

After this, we're ready to `cpio` the whole initramfs up. This `cpio` command will (g)zip up all of our
files and folders up into something the kernel can work with:

    find . -print0 \
        | cpio --null -ov --format=newc \
        | gzip -9 > ~/dev/initramfs/initramfs-busybox-x86.cpio.gz

### Booting

Ah, for the moment of truth. Truth to be told, it's a rather underwhelming command. But the results are
exciting!

    qemu-system-x86_64 \
        -kernel ~/dev/linux/arch/x86_64/boot/bzImage \
        -initrd ~/dev/initramfs/initramfs-busybox-x86.cpio.gz \
        -enable-kvm

Notice how we use those fancy kvm settings after all. The qemu command should pop open a small window
with a small Linux environment running inside. All your classic userspace utilities should be working
courtesy of busybox.


### Addendums

I used a lot of resources on the internet to arrive at this process, [but one resource stands out in
particular][3].


[0]: https://github.com/torvalds/linux
[1]: https://en.wikipedia.org/wiki/Initramfs
[2]: https://git.busybox.net/busybox
[3]: https://mgalgs.github.io/2015/05/16/how-to-build-a-custom-linux-kernel-for-qemu-2015-edition.html
