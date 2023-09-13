% AppImage explosions

In a [previous post][0], I detailed how bpftrace’s CI was reworked to use Nix.
As part of that effort, the project-distributed semi-static binary was replaced
with an appimage. In this post I dive into an issue I had to debug.

### The problem

The first thing I did after building the appimage was try to run it.  `./result
--help` worked as expected. `sudo bpftrace --info` (a quick and dirty way to
validate end to end functionality) seemed to work as well. But shortly after,
while trying to open a new terminal, I was getting `unable to allocate pty: No
such device` errors.  This resulted in a bit of head scratching.  I don’t
remember how I figured it out, but eventually I noticed the mount table was all
messed up.

To be more specific, here is what was happening:

```
[vagrant@fedora vagrant]$ mount
proc on /proc type proc (rw,nosuid,nodev,noexec,relatime)
sysfs on /sys type sysfs (rw,nosuid,nodev,noexec,relatime,seclabel)
devtmpfs on /dev type devtmpfs (rw,nosuid,seclabel,size=997040k,nr_inodes=249260,mode=755,inode64)
securityfs on /sys/kernel/security type securityfs (rw,nosuid,nodev,noexec,relatime)
tmpfs on /dev/shm type tmpfs (rw,nosuid,nodev,seclabel,inode64)
devpts on /dev/pts type devpts (rw,nosuid,noexec,relatime,seclabel,gid=5,mode=620,ptmxmode=000)
tmpfs on /run type tmpfs (rw,nosuid,nodev,seclabel,size=404844k,nr_inodes=819200,mode=755,inode64)
cgroup2 on /sys/fs/cgroup type cgroup2 (rw,nosuid,nodev,noexec,relatime,seclabel,nsdelegate,memory_recursiveprot)
pstore on /sys/fs/pstore type pstore (rw,nosuid,nodev,noexec,relatime,seclabel)
none on /sys/fs/bpf type bpf (rw,nosuid,nodev,noexec,relatime,mode=700)
/dev/sda1 on / type ext4 (rw,relatime,seclabel)
selinuxfs on /sys/fs/selinux type selinuxfs (rw,nosuid,noexec,relatime)
systemd-1 on /proc/sys/fs/binfmt_misc type autofs (rw,relatime,fd=36,pgrp=1,timeout=0,minproto=5,maxproto=5,direct,pipe_ino=14870)
mqueue on /dev/mqueue type mqueue (rw,nosuid,nodev,noexec,relatime,seclabel)
debugfs on /sys/kernel/debug type debugfs (rw,nosuid,nodev,noexec,relatime,seclabel)
tracefs on /sys/kernel/tracing type tracefs (rw,nosuid,nodev,noexec,relatime,seclabel)
hugetlbfs on /dev/hugepages type hugetlbfs (rw,relatime,seclabel,pagesize=2M)
tmpfs on /tmp type tmpfs (rw,nosuid,nodev,seclabel,nr_inodes=409600,inode64)
fusectl on /sys/fs/fuse/connections type fusectl (rw,nosuid,nodev,noexec,relatime)
configfs on /sys/kernel/config type configfs (rw,nosuid,nodev,noexec,relatime)
tmpfs on /run/user/1000 type tmpfs (rw,nosuid,nodev,relatime,seclabel,size=202420k,nr_inodes=50605,mode=700,uid=1000,gid=1000,inode64)

[vagrant@fedora vagrant]$ sudo ./result --info &>/dev/null

[vagrant@fedora vagrant]$ mount
proc on /proc type proc (rw,nosuid,nodev,noexec,relatime)
sysfs on /sys type sysfs (rw,nosuid,nodev,noexec,relatime,seclabel)
devtmpfs on /dev type devtmpfs (rw,nosuid,seclabel,size=997040k,nr_inodes=249260,mode=755,inode64)
tmpfs on /run type tmpfs (rw,nosuid,nodev,seclabel,size=404844k,nr_inodes=819200,mode=755,inode64)
/dev/sda1 on / type ext4 (rw,relatime,seclabel)
tmpfs on /tmp type tmpfs (rw,nosuid,nodev,seclabel,nr_inodes=409600,inode64)
```

Obviously this is not ok. `bpftrace` tries very hard not to mess up the host.
After all, one of it its selling points is that you can safely use it on a
production host.

### What’s an AppImage anyways?

To the user, an appimage is a binary that looks and feels like a statically
linked binary. In fact, you’d have a hard time discovering a binary is an
appimage at all.

At its core, an appimage is a squashfs image that contains an application and
all of its dependencies. Prepended to the binary is a statically linked runtime
that:

1. Knows where the squashfs image starts
1. Mounts the squashfs image via squashfuse
1. Transfers control to (yet another statically linked) entrypoint binary inside the squashfs mount

In the case of `nix-appimage`, this entrypoint binary creates a new user and
mount namespace. The user namespace is necessary to perform privileged
operations like mounting. If the appimage is being run as root, the user
namespace may be omitted.

Inside the new mount namespace, the entrypoint binary then:

1. Mounts a tmpfs at the "mountroot"
1. Bind mounts everything in the hosts’s root directory into `$mountroot`
1. Bind mounts the embedded nix store to `$mountroot/nix/store`
1. Chroot to the mountroot
1. Transfers control to the (possibly dynamically linked) application in the
   Nix store

In this way, the application's dynamic linker can "just find" the shared
libraries without any additional `rpath` manipulation.

As you can see, appimages integrate quite elegantly with Nix — it’s fairly
trivial with Nix to query and embed a derivation and all its dependencies into
a squashfs image.

### Everyone loves printf

Back to the problem. The good news is that both the appimage runtime and
entrypoint binary amount to about 1000 lines of fairly straightforward C. So it
is easy to both debug and maintain if the need arises.

Since the mount logic was getting fancy, I figured it was a good place to start
debugging. After a few choice placements of `system("cat /proc/self/mountinfo |
column -t")`s, I got the following dump:


```
/            /                                                                                 shared:1
/            /sys                                                                              shared:2
/            /sys/kernel/security                                                              shared:3
/            /sys/fs/cgroup                                                                    shared:4
/            /sys/fs/pstore                                                                    shared:5
/            /sys/fs/bpf                                                                       shared:6
/            /sys/fs/selinux                                                                   shared:7
/            /sys/kernel/tracing                                                               shared:16
/            /sys/kernel/debug                                                                 shared:18
/            /sys/kernel/config                                                                shared:19
/            /sys/fs/fuse/connections                                                          shared:20
/            /dev                                                                              shared:8
/            /dev/shm                                                                          shared:9
/            /dev/pts                                                                          shared:10
/            /dev/hugepages                                                                    shared:14
/            /dev/mqueue                                                                       shared:15
/            /run                                                                              shared:11
/            /run/user/1000                                                                    shared:210
/            /proc                                                                             shared:12
/            /proc/sys/fs/binfmt_misc                                                          shared:13
/            /tmp                                                                              shared:17
/            /tmp/.FLCABm                                                                      shared:255
/            /tmp/.FLCABm/mountroot                                                            shared:350
/            /tmp/.FLCABm/mountroot/proc                                                       shared:12
/            /tmp/.FLCABm/mountroot/proc/sys/fs/binfmt_misc                                    shared:13
/usr/bin     /tmp/.FLCABm/mountroot/bin                                                        shared:1
/            /tmp/.FLCABm/mountroot/run                                                        shared:11
/            /tmp/.FLCABm/mountroot/run/user/1000                                              shared:210
/root        /tmp/.FLCABm/mountroot/root                                                       shared:1
/usr/lib64   /tmp/.FLCABm/mountroot/lib64                                                      shared:1
/media       /tmp/.FLCABm/mountroot/media                                                      shared:1
/home        /tmp/.FLCABm/mountroot/home                                                       shared:1
/vagrant     /tmp/.FLCABm/mountroot/vagrant                                                    shared:1
/opt         /tmp/.FLCABm/mountroot/opt                                                        shared:1
/mnt         /tmp/.FLCABm/mountroot/mnt                                                        shared:1
/lost+found  /tmp/.FLCABm/mountroot/lost+found                                                 shared:1
/srv         /tmp/.FLCABm/mountroot/srv                                                        shared:1
/            /tmp/.FLCABm/mountroot/sys                                                        shared:2
/            /tmp/.FLCABm/mountroot/sys/kernel/security                                        shared:3
/            /tmp/.FLCABm/mountroot/sys/fs/cgroup                                              shared:4
/            /tmp/.FLCABm/mountroot/sys/fs/pstore                                              shared:5
/            /tmp/.FLCABm/mountroot/sys/fs/bpf                                                 shared:6
/            /tmp/.FLCABm/mountroot/sys/fs/selinux                                             shared:7
/            /tmp/.FLCABm/mountroot/sys/kernel/tracing                                         shared:16
/            /tmp/.FLCABm/mountroot/sys/kernel/debug                                           shared:18
/            /tmp/.FLCABm/mountroot/sys/kernel/config                                          shared:19
/            /tmp/.FLCABm/mountroot/sys/fs/fuse/connections                                    shared:20
/usr         /tmp/.FLCABm/mountroot/usr                                                        shared:1
/            /tmp/.FLCABm/mountroot/tmp                                                        shared:17
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm                                    shared:255
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot                          shared:350
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/proc                     shared:12
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/proc/sys/fs/binfmt_misc  shared:13
/usr/bin     /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/bin                      shared:1
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/run                      shared:11
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/run/user/1000            shared:210
/root        /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/root                     shared:1
/usr/lib64   /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/lib64                    shared:1
/media       /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/media                    shared:1
/home        /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/home                     shared:1
/vagrant     /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/vagrant                  shared:1
/opt         /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/opt                      shared:1
/mnt         /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/mnt                      shared:1
/lost+found  /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/lost+found               shared:1
/srv         /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/srv                      shared:1
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/sys                      shared:2
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/sys/kernel/security      shared:3
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/sys/fs/cgroup            shared:4
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/sys/fs/pstore            shared:5
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/sys/fs/bpf               shared:6
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/sys/fs/selinux           shared:7
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/sys/kernel/tracing       shared:16
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/sys/kernel/debug         shared:18
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/sys/kernel/config        shared:19
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/sys/fs/fuse/connections  shared:20
/usr         /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/usr                      shared:1
/            /tmp/.FLCABm/mountroot/dev                                                        shared:8
/            /tmp/.FLCABm/mountroot/dev/shm                                                    shared:9
/            /tmp/.FLCABm/mountroot/dev/pts                                                    shared:10
/            /tmp/.FLCABm/mountroot/dev/hugepages                                              shared:14
/            /tmp/.FLCABm/mountroot/dev/mqueue                                                 shared:15
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/dev                      shared:8
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/dev/shm                  shared:9
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/dev/pts                  shared:10
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/dev/hugepages            shared:14
/            /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/dev/mqueue               shared:15
/var         /tmp/.FLCABm/mountroot/var                                                        shared:1
/var         /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/var                      shared:1
/usr/sbin    /tmp/.FLCABm/mountroot/sbin                                                       shared:1
/usr/sbin    /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/sbin                     shared:1
/etc         /tmp/.FLCABm/mountroot/etc                                                        shared:1
/etc         /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/etc                      shared:1
/boot        /tmp/.FLCABm/mountroot/boot                                                       shared:1
/boot        /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/boot                     shared:1
/usr/lib     /tmp/.FLCABm/mountroot/lib                                                        shared:1
/usr/lib     /tmp/.FLCABm/mountroot/tmp/.mount_resultFLCABm/mountroot/lib                      shared:1
```

(Note that I've trimmed some of the columns to make the output more legible.)

There are two problems to notice here.

First is that we are sharing quite a few mounts with the root namespace in peer
group 1 (see `shared:1` at the end of the mount entries).

Second is the "mount explosion" problem, as described in
[`mount_namespaces(7)`][1]. By virtue of iterating through the root namespace’s
`/`, the entrypoint binary is recursively binding the root namespace’s mounts
into the new mount namespace over and over again.

### Problem 1

According to `mount_namespaces(7)`:

> This mount shares events with members of a peer group. mount(2) and umount(2)
> events immediately under this mount will propagate to the other mounts that
> are members of the peer group.  Propagation here means that the same mount(2)
> or umount(2) will automatically occur under all of the other mounts in the
> peer group.  Conversely, mount(2) and umount(2) events that take place under
> peer mounts will propagate to this mount.

In other words, if a mount `M` is shared in a peer group, any events that occur
_under_ `M` are propagated to all peers. This explains the host’s mounts were
getting messed up after the appimage exits. When the appimage exits, its FUSE
mount is cleaned up. This leads to all the mounts under the FUSE being
unmounted. And since these lower mounts are shared, the umount event is
propagated to the root namespace. Note that this would only occur for appimages
run as root — an unprivileged user namespace would not have sufficient
permissions to umount mounts from the root namespace.

As for why the system doesn’t immediately crash, presumably mounts which have
any of their files still opened are not automatically umounted.

The fix is to rebind the mountroot as `MS_PRIVATE`. This prevents events from
under the mountroot from propagating out.

### Problem 2

The second problem is the “mount explosion”.  Granted, this is not a critically
important problem. It is, however, kinda ugly and probably wastes resources.
Fortunately the fix is simple: use `MS_UNBINDABLE` to prevent recursive mount
explosion problems.

But what about the previous `MS_PRIVATE` fix? The good news is that the VFS
developers have thought of that already! `MS_UNBINDABLE` is simply a more
powerful `MS_PRIVATE`. Put differently: `MS_UNBINDABLE` is strictly additive.

Two birds with one stone, as they say.

### Conclusion

Appimages are quite an elegant mechanism in my opinion. They attempt solve a
difficult problem — how we ship increasingly complex software. And they do it
with relative simplicity w.r.t. lines of code. While this bug was a little
tricky to debug, it didn’t take much time at all b/c I could hold all the code
in my head.


[0]: https://dxuuu.xyz/bpftrace-nix.html
[1]: https://www.man7.org/linux/man-pages/man7/mount_namespaces.7.html
