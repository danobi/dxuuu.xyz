% Understanding btrfs internals part 5

This is the fifth and final part of a multipart series that explains the basics
of [btrfs][0]'s on-disk format.

At the end of this series, we'll have a program that can print out the absolute
path of every regular file in an unmounted btrfs filesystem image without
external libraries or `ioctl(2)` calls.

Example code is available [here][1].

### Background

At the end of [Part 4][2], we had access to the filesystem tree root. All
that's left for us to do now is walk the filesystem tree and make sense of the
data it contains.

By now, you should be fairly familiar with the high level B-tree algorithms
we're using. You should also be familiar with how btrfs structures its
metadata. The hard part is over! This final part is not very complicated --
at this point it's an exercise in reading [the documentation][3].

### Filesystem tree item types

There are quite a few item types stored in the FS tree. However, we only care
about two:

1. [`BtrfsDirItem`][4]
2. [`BtrfsInodeRef`][5]

`BtrfsDirItem` represents an entry in a directory. The name of the directory entry
directly follows the structure. We'll enumerate all the `BtrfsDirItem`s in the
filesystem, grab their names, and the compute the absolute path leading up to the
directory entry.

`BtrfsInodeRef` is a helper structure that helps link inode numbers to
`BtrfsDirItem`s.  It also contains information on the parent of the inode.
We'll use this information to locate the parents for every regular file we
find. The name of the inode `BtrfsInodeRef` refers to directly follows the
structure.

`BtrfsDirItem` and `BtrfsInodeRef` are defined as follows:

```{#function .rust}
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct BtrfsDirItem {
    pub location: BtrfsKey,
    pub transid: u64,
    pub data_len: u16,
    pub name_len: u16,
    pub ty: u8,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct BtrfsInodeRef {
    pub index: u64,
    pub name_len: u16,
}
```

### Writing the code

```{#function .rust}
fn walk_fs_tree(
    file: &File,
    superblock: &BtrfsSuperblock,
    node: &[u8],
    root_fs_node: &[u8],
    cache: &ChunkTreeCache,
) -> Result<()> {
    let header = tree::parse_btrfs_header(node)?;
```

This should be fairly familiar to you by now. Always parse the header of a node
first.

```{#function .rust}
    // Leaf node
    if header.level == 0 {
        let items = tree::parse_btrfs_leaf(node)?;
        for item in items {
            if item.key.ty != BTRFS_DIR_ITEM_KEY {
                continue;
            }

            let dir_item = unsafe {
                &*(node
                    .as_ptr()
                    .add(std::mem::size_of::<BtrfsHeader>() + item.offset as usize)
                    as *const BtrfsDirItem)
            };

            if dir_item.ty != BTRFS_FT_REG_FILE {
                continue;
            }
```

This too should be familiar. If we're at a leaf node, start processing the
items in the node.

For our use case, we only want `BtrfsDirItem`s that represent regular files.
Skip everything else.

```{#function .rust}
            let name_slice = unsafe {
                std::slice::from_raw_parts(
                    (dir_item as *const BtrfsDirItem as *const u8)
                        .add(std::mem::size_of::<BtrfsDirItem>()),
                    dir_item.name_len.into(),
                )
            };
            let name = std::str::from_utf8(name_slice)?;
```

Extract the name of the directory entry. For example, if this `BtrfsDirItem`
represented `/home/daniel/dev/readme.txt`, `name` would contain `readme.txt`.

Now that we have the filename, we must compute the absolute path leading up to
it.

```{#function .rust}
            // Capacity 1 so we don't panic the first `String::insert`
            let mut path_prefix = String::with_capacity(1);
            // `item.key.objectid` is parent inode number
            let mut current_inode_nr = item.key.objectid;
```

Here's where things get a little trickier. If you haven't already noticed,
btrfs tends to store whatever it wants inside a `BtrfsKey`. The only thing
that's really nailed down is `BtrfsKey::ty`. Aside from the type field, the
meaning of the `BtrfsKey::objectid` and `BtrfsKey::offset` fields completely
depend on the item type. In other words, just because the name is `offset`
doesn't necessarily mean it's actually the offset to anything.

Here, `item.key.objectid` actually means the inode number of the current
directory item's parent.

```{#function .rust}
            loop {
```

Start a loop that'll end when we're done looking up the absolute path
of this current directory entry.

```{#function .rust}
                let (current_key, _current_inode, current_inode_payload) =
                    get_inode_ref(current_inode_nr, file, superblock, root_fs_node, cache)?
                        .ok_or_else(|| {
                            anyhow!("Failed to find inode_ref for inode={}",
                                current_inode_nr)
                        })?;
                unsafe { assert_eq!(current_key.objectid, current_inode_nr) };
```

Look up the `BtrfsInodeRef` for the `current_inode_nr`. When the loop beings,
it holds the immediate parent to the directory entry. As the loop iterates,
it'll go to the parent's parent, the parent's parent's parent, etc. until we
reach the root of the filesystem.

`get_inode_ref()` returns a tuple of:

* `BtrfsKey` associated with the `BtrfsInodeRef`
* the `BtrfsInodeRef` struct itself
* the payload after the `BtrfsInodeRef`

We omit `get_inode_ref()`s implementation for brevity. It's essentially the
same code as this function except it searches for `BtrfsInodeRef`s.

```{#function .rust}
                if current_key.offset == current_inode_nr {
                    path_prefix.insert(0, '/');
                    break;
                }
```

Check if we've reached the root of the filesystem. For `BtrfsInodeRef`s, the
`BtrfsKey::offset` field holds `_current_inode`'s parent's inode number. If the
parent inode # and the current inode # match, it means we're at the root of the
filesyste.

If we're at the root, insert a `/` to `path_prefix` to root the absolute path
and exit the loop.

```{#function .rust}
                path_prefix.insert_str(
                    0,
                    &format!("{}/", std::str::from_utf8(&current_inode_payload)?),
                );
```

If we're not yet at the root, we interpret `BtrfsInodeRef`'s payload as a
string containing the name of the inode. We tack that onto the front of
`path_prefix`.

```{#function .rust}
                current_inode_nr = current_key.offset;
            }
```

The last thing we do in the loop is set `current_inode_nr` to its parent's
inode number. This ensure we keep moving closer to the root of the filesystem.

```{#function .rust}
            println!("filename={}{}", path_prefix, name);
        }
```

Finally, after the loop exits, we combine `path_prefix` and `name` to get the
absolute path of the regular file we're processing. We print the result to the
terminal.

```{#function .rust}
    } else {
        let ptrs = tree::parse_btrfs_node(node)?;
        for ptr in ptrs {
            let physical = cache
                .offset(ptr.blockptr)
                .ok_or_else(|| anyhow!("fs tree node not mapped"))?;
            let mut node = vec![0; superblock.node_size as usize];
            file.read_exact_at(&mut node, physical)?;
            walk_fs_tree(file, superblock, &node, root_fs_node, cache)?;
        }
    }
```

This really ought to bore you now. If we're at an internal node, recursively
process each of the child nodes.


```{#function .rust}
    Ok(())
}
```

And finally, if we've reached the end of the function, we're done.

### Conclusion

Congrats on making it this far in the series! Creating and writing up
`btrfs-walk` was extraordinarly educational for me and hopefully for you as
well.

Note that we've just barely scratched the surface of all the complexity btrfs
contains. But with a solid understanding of the core data structures, you
should feel comfortable diving deeper into btrfs internals on your own.

If you're still interested in btrfs, take a look at [`btrfs-fuzz`][6]. It's an
unsupervised coverage-guided btrfs fuzzer that I'm creating using what I've
learned from btrfs-walk.


[0]: https://en.wikipedia.org/wiki/Btrfs
[1]: https://github.com/danobi/btrfs-walk
[2]: btrfs-internals-4.html
[3]: https://btrfs.wiki.kernel.org/index.php/On-disk_Format#FS_TREE_.285.29
[4]: https://btrfs.wiki.kernel.org/index.php/Data_Structures#btrfs_dir_item
[5]: https://btrfs.wiki.kernel.org/index.php/Data_Structures#btrfs_inode_ref
[6]: https://github.com/danobi/btrfs-fuzz
