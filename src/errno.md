% errno and libc

A few weeks ago I wanted to confirm if `errno` was a libc abstraction or a
kernel feature. The glibc docs are deliberately vague on the topic, so
experimentation seemed like the best course.

### Using libc

```c
static int use_wrapper(int cmd, union bpf_attr *attr, unsigned int size)
{
	long ret;

	/* Clear errno */
	errno = 0;

	ret = syscall(__NR_bpf, cmd, attr, size);

	if (ret < 0)
		printf("wrapped syscall failed, ret=%d, errno=%d\n", ret, errno);
	else
		printf("wrapped syscall succeeded\n");
}
```

Here we make a syscall to `bpf(2)`. Mostly b/c I'm quite familiar with it.

### Using assembly

```c
static int use_raw(int cmd, union bpf_attr *attr, unsigned int size)
{
	long ret;

	/* Clear errno */
	errno = 0;

	__asm__(
		"movq %1, %%rax\n"        /* syscall number */
		"movq %2, %%rdi\n"        /* arg1 */
		"movq %3, %%rsi\n"        /* arg2 */
		"movq %4, %%rdx\n"        /* arg3 */
		"syscall\n"
		"movq %%rax, %0\n"

		/* retval */
		: "=r"(ret)

		/* input operands */
		: "r"((long)__NR_bpf), "r"((long)cmd), "r"((long)attr), "r"((long)size)

		/* clobbers */
		: "rax", "rdi", "rsi", "rdx"
       );

	/* Check return value */
	if (ret < 0)
		printf("raw syscall failed, ret=%d, errno=%d\n", ret, errno);
	else
		printf("raw syscall succeeded\n");
}
```

`use_raw()` does the exact same thing as `use_wrapper()`, except we use inline
assembly. Even if you don't know assembly or inline assembler, it should be
quite clear the semantics from the comments.

### Trying it out

```c
#include <sys/syscall.h>
#include <unistd.h>
#include <stdio.h>
#include <errno.h>
#include <linux/bpf.h>

[..]

int main()
{
	int cmd = BPF_PROG_LOAD;
	union bpf_attr *attr = NULL;
	unsigned int size = 0;

	use_raw(cmd, attr, size);
	use_wrapper(cmd, attr, size);
}
```

Finally, we put it together in `main()`. Consider this output:

```
$ gcc main.c

$ ./a.out
raw syscall failed, ret=-1, errno=0
wrapped syscall failed, ret=-1, errno=1

$ sudo ./a.out
raw syscall failed, ret=-7, errno=0
wrapped syscall failed, ret=-1, errno=7
```

Notice how `errno` is always 0 when skiping libc. Further notice how the return
value of the raw syscall flipped to a positive value and stored into `errno` by
libc.

So that settles it -- `errno` is a libc abstraction.
