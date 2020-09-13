% Understanding btrfs internals

This is the first of a multipart series that explains the basics of
[btrfs][0]'s on-disk format.

At the end of this series, we'll have a program that can print out the absolute
path of every regular file in an unmounted btrfs filesystem image without
external libraries or `ioctl(2)` calls.

Example code is available [here][5].

### Background

Before we begin, it might be helpful to go over some filesystem basics. First,
there's [block devices][1]. Block devices are a level of abstraction over
physical (or virtual) hardware. You can think of a block device as a linear
series of bytes, of which you can read or write at any offset or size.
Filesystems are typically built on top of block devices and offers structure
and other very useful functionality for storing your data. Such functionality
might include files, directories, data checksums, striping over multiple block
devices, compression, etc. To support these features, filesystems need to store
metadata on the block device in addition to user data.

Second, what you typically think of when I say "filesystem" is probably a
[POSIX][2] filesystem. A POSIX filesystem has all the unix-y things we've come
to love: files, "." and ".." directories, a filesystem tree, as well as the
standard APIs (eg `read()`, `write()`, `lseek()`, etc). If the APIs are
implemented as system calls (they usually are), the kernel has to be filesystem
aware. For linux, most filesystems are either compiled into the kernel or
loaded as kernel modules.

Third, a filesystem image can be thought of as the contents of the entire block
device a filesystem is in charge of. It's usually not a great idea to manually
modify a filesystem image. However, it's totally safe to read from a mounted or
unmounted one.  "Safe" here means you probably won't corrupt any data but you
might get inconsistent data if you read from a mounted image (b/c the kernel
could be making changes).

### Notes

A couple notes to keep in mind while reading:

* The code samples here may not compile -- certain necessary boilerplate may be
  omitted in the interest of legibility

* There's a lot of uninteresting details I'll skip over (as well as interesting
  details, but unfortunately I can't cover it all)


### Creating a btrfs image

Instead of a "real" block device with a real physical drive behind it, we'll
use a fake block device (a loopback) to keep things simple (or even more
complicated, if you know how a loopback device works under the hood). To create
a loopback btrfs image and mount it, run:

```shell
$ truncate -s 1G image

$ mkfs.btrfs image
btrfs-progs v5.7
See http://btrfs.wiki.kernel.org for more information.

Label:              (null)
UUID:               a32cd5e8-2729-4281-b41b-153ea353ffd3
Node size:          16384
Sector size:        4096
Filesystem size:    1.00GiB
Block group profiles:
  Data:             single            8.00MiB
  Metadata:         DUP              51.19MiB
  System:           DUP               8.00MiB
SSD detected:       no
Incompat features:  extref, skinny-metadata
Runtime features:
Checksum:           crc32c
Number of devices:  1
Devices:
   ID        SIZE  PATH
    1     1.00GiB  image

$ sudo mkdir /mnt/btrfs

$ sudo mount image /mnt/btrfs

$ findmnt /mnt/btrfs
TARGET     SOURCE     FSTYPE OPTIONS
/mnt/btrfs /dev/loop0 btrfs  rw,relatime,ssd,space_cache,subvolid=5,subvol=/
```

Note that `image` is a regular file with some bytes in it. If you choose to
leave the image mounted, remember to run `sync` after modifying anything in the
filesystem so that the changes are persisted to "disk", or in our case, our
`image` file.

### Parsing the superblock

The superblock is the starting point of any filesystem. It's a structure of
predefined size written to a predefined location inside the filesystem image.
The key property is that it has all the information necessary to bootstrap and
initialize filesystem data structures.

First we'll define the on-disk superblock structure:

``` {#function .rust}
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct BtrfsSuperblock {
    pub csum: [u8; BTRFS_CSUM_SIZE],
    pub fsid: [u8; BTRFS_FSID_SIZE],
    /// Physical address of this block
    pub bytenr: u64,
    pub flags: u64,
    pub magic: [u8; 0x8],
    pub generation: u64,
    /// Logical address of the root tree root
    pub root: u64,
    /// Logical address of the chunk tree root
    pub chunk_root: u64,
    /// Logical address of the log tree root
    pub log_root: u64,
    pub log_root_transid: u64,
    pub total_bytes: u64,
    pub bytes_used: u64,
    pub root_dir_objectid: u64,
    pub num_devices: u64,
    pub sector_size: u32,
    pub node_size: u32,
    /// Unused and must be equal to `nodesize`
    pub leafsize: u32,
    pub stripesize: u32,
    pub sys_chunk_array_size: u32,
    pub chunk_root_generation: u64,
    pub compat_flags: u64,
    pub compat_ro_flags: u64,
    pub incompat_flags: u64,
    pub csum_type: u16,
    pub root_level: u8,
    pub chunk_root_level: u8,
    pub log_root_level: u8,
    pub dev_item: BtrfsDevItem,
    pub label: [u8; BTRFS_LABEL_SIZE],
    pub cache_generation: u64,
    pub uuid_tree_generation: u64,
    pub metadata_uuid: [u8; BTRFS_FSID_SIZE],
    /// Future expansion
    pub _reserved: [u64; 28],
    pub sys_chunk_array: [u8; BTRFS_SYSTEM_CHUNK_ARRAY_SIZE],
    pub root_backups: [BtrfsRootBackup; 4],
}
```

To parse the superblock, we write the following code:

``` {#function .rust}
const BTRFS_SUPERBLOCK_OFFSET: u64 = 0x10_000;
```

The first [btrfs superblock][3] (of possibly 3) starts at offset `0x10000`.

``` {#function .rust}
const BTRFS_SUPERBLOCK_MAGIC: [u8; 8] = *b"_BHRfS_M";
```

Most superblocks have a "magic" value embedded inside so that a filesystem
implementation has a way to easily identify that the image it's been told to
process is a format it can understand.

``` {#function .rust}
fn parse_superblock(file: &File) -> Result<BtrfsSuperblock> {
    let mut superblock: BtrfsSuperblock = unsafe { std::mem::zeroed() };
    let superblock_size = std::mem::size_of::<BtrfsSuperblock>();
    let slice;
    unsafe {
        slice = slice::from_raw_parts_mut(&mut superblock as *mut _ as *mut u8, superblock_size);
    }
    file.read_exact_at(slice, BTRFS_SUPERBLOCK_OFFSET)?;
    if superblock.magic != BTRFS_SUPERBLOCK_MAGIC {
        bail!("superblock magic is wrong");
    }

    Ok(superblock)
}
```

This function takes our `image` file as a borrowed [`File`][4] and returns a
`BtrfsSuperblock`. Then we do a bit of unsafe rust to read from the right
offset `sizeof(BtrfsSuperblock)` bytes. If, after reading, the magic doesn't
match, we bail with an error. The superblock technically contains a `csum`
(short for checksum) value and we could (and probably should) check that the
checksum for the superblock matches, but we're lazy and verifying the magic
value is probably good enough.

### Next

Now that we have the superblock, we can start bootstrapping the rest of the
data structures. More in the next post.

[0]: https://en.wikipedia.org/wiki/Btrfs
[1]: https://en.wikipedia.org/wiki/Device_file#BLOCKDEV
[2]: https://en.wikipedia.org/wiki/POSIX
[3]: https://btrfs.wiki.kernel.org/index.php/On-disk_Format#Superblock
[4]: https://doc.rust-lang.org/std/fs/struct.File.html
[5]: https://github.com/danobi/btrfs-walk
