% Truthiness in C

This week someone mentioned that C supports casting from `int` to `bool`.  That
naturally triggered my curiosity -- what does the generated code look like?

First I think it's important to point out most casts in C are "free". "Free" in
the sense that the compiler changes its internal understanding of an
expression. But in actual assembly nothing really changes -- registers don't
have types.

For example, consider:

```c
char a(int x)
{
    return (char)x;
}

int b(char x)
{
    return (int)x;
}

char *c(int *x)
{
    return (char *)x;
}
```

The generated code looks like:

```
a(int):
        mov     eax, edi
        ret
b(char):
        movsx   eax, dil
        ret
c(int*):
        mov     rax, rdi
        ret
```

Clearly we are just moving values between the argument register and the return
register. So the cast is "free".

But what about casting from `int` to `bool`?

```c
bool d(int x)
{
    return (bool)x;
}
```

Well, the compiler gives us:

```
d(int):
        test    edi, edi
        setne   al
        ret
```

We see that it generates code to first `test eax` and then to `setne al`.
[`test`][0] is used to set the status flags. [`setne`][1] is used to set the
lowest 8 bit subregister in `rax`. `rax` is used to [return integer values][2]
from a function in the System V ABI.

Note that the top 56 bits in `rax` are not zeroed -- they contain junk. This is
fine b/c the compiler will only make callers check the lowest bit of a register
for boolean operations. This is why changing the compiler's "understanding" (ie
the cast) is necessary.

So, two otherwise extra instructions. Not too bad for how useful it is.

See the full code [here][3].

### Errata

Thanks thxg on HN for pointing out that I was incorrectly compiling with `-m32`
but talking about x86-64.


[0]: https://web.itu.edu.tr/kesgin/mul06/intel/instr/test.html
[1]: https://web.itu.edu.tr/kesgin/mul06/intel/instr/setne_setnz.html
[2]: https://en.wikipedia.org/wiki/X86_calling_conventions#System_V_AMD64_ABI
[3]: https://godbolt.org/z/ff8r44nKn
