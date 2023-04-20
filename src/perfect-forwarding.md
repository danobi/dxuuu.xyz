% Not so perfect forwarding

Consider the following C++ structs:

```
struct Foo {
  char b;
  int a;
} __attribute__((packed));

struct Bar {
  Bar(int a) {}
};
```

Now consider the following code:

```
#include <memory>

int main() {
  Foo f;
  auto p = std::make_unique<Bar>(f.a);
}
```

It should compile right? Wrong:

```
/tmp/cppsh-NLnGU.cpp: In function ‘int main()’:
/tmp/cppsh-NLnGU.cpp:14:36: error: cannot bind packed field ‘f.Foo::a’ to ‘int&’
   14 |   auto p = std::make_unique<Bar>(f.a);
      |
```

What gives? I can take it on faith that references to packed fields are banned
-- in general unaligned memory access is bad. But `Bar::Bar` takes an `int` by
value, not by reference.

The answer lies in `std::make_unique`. `std::make_unique` uses perfect
forwarding, a way to forward function arguments "without changing its lvalue or
rvalue characteristics" such as `const`-ness and rvalue-ness. Its function
signature looks like this:

```
template< class T, class... Args >
unique_ptr<T> make_unique( Args&&... args );
```

In order to forward the perfectly forward the arguments without extra copies,
it tries to take a reference, leaving the final (and required) copy to
`Bar::Bar`. Usually this is good, but in this specific case it breaks our code.

To workaround this issue, we could do:

```
int main() {
  Foo f;
  int a = f.a;
  auto p = std::make_unique<Bar>(a);
}
```

### Addendum

Scott Meyers actually lays out a [few more][0] perfect forwarding failure cases. Packed
members are not covered, but bitfields are probably close enough semantically.


[0]: https://github.com/peter-can-write/cpp-notes/blob/master/perfect-forwarding-failure-cases.md
