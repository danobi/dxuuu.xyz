% std::vector and emplace_back

There are some interesting bits about the C++ runtime. Consider this problem:
you want to add an element to a vector of type `std::vector<std::pair<int, Foo>>`
with as little overhead as possible. That means one construction -- no extra moves,
no extra copies, nothing.

For this problem, let us use this implementation of Foo:

``` {#function .cpp .numberLines startFrom="1"}
class Foo {
public:
    Foo() {
        std::cout << "constructor\n";
    }

    Foo(int x) {
        std::cout << "constructor2\n";
    }

    Foo(const Foo& f) {
        std::cout << "copy constructor\n";
    }

    Foo(Foo&& f) {
        std::cout << "move constructor\n";
    }
};
```

Obviously we want to use `emplace_back` to elide the move you would traditionally
get from something like `vec.push_back(std::move(f))`.

``` {#function .cpp .numberLines startFrom="1"}
int main()
{
  std::vector<std::pair<int, Foo>> list;
  list.emplace_back(3, Foo{});
}
``

When run:
```
$ ./a.out
constructor
move constructor
```

So what happened? Clearly `Foo` was `std::move`d at least once. In fact, it happens
when `emplace_back` is evaluated, since we constructed an rvalue `Foo{}`.

Ok, so we need a different approach. How about forwarding through the arguments to `Foo`,
so `Foo` is constructed as late as possible?

``` {#function .cpp .numberLines startFrom="1"}
#include <tuple>

int main()
{
  std::vector<std::pair<int, Foo>> list;
  list.emplace_back(std::piecewise_construct, std::forward_as_tuple(3), std::forward_as_tuple());
}
```

```
$ ./a.out
constructor
```

Here we use `std::forward_as_tuple` to tell the compiler that we want to call the 0-arg constructor.
If using fancy C++ standard library features isn't your cup of tea, you could alternatively use
a dummy constructor:

``` {#function .cpp .numberLines startFrom="1"}
int main()
{
  std::vector<std::pair<int, Foo>> list;
  list.emplace_back(3, 1);
}
```

```
$ ./a.out
constructor2
```

Great! So we solved our original problem. Or did we? Consider this:

``` {#function .cpp .numberLines startFrom="1"}
int main()
{
  std::vector<std::pair<int, Foo>> list;
  list.emplace_back(3, 1);
  std::cout << "---\n";
  list.emplace_back(3, Foo{});
  std::cout << "---\n";
  list.emplace_back(3, 1);
}
```

```
$ ./a.out
constructor2
---
constructor
move constructor
copy constructor
---
constructor2
copy constructor
copy constructor
```

Woah, what's with all the extra copies? Well, as it turns out, when a vector needs
to be resized, enough contiguous memory for all elements needs to be allocated. Then,
the contents of the old vector need to be move or copy constructed into the new
memory. To elide this issue, we can preallocate memory for our vector.

``` {#function .cpp .numberLines startFrom="1"}
int main()
{
  std::vector<std::pair<int, Foo>> list;
  list.reserve(3);
  list.emplace_back(3, 1);
  std::cout << "---\n";
  list.emplace_back(3, Foo{});
  std::cout << "---\n";
  list.emplace_back(3, 1);
}
```

```
$ ./a.out
constructor2
---
constructor
move constructor
---
constructor2
```

Hey, now this is Looking good. But wait! There's one outstanding question: what's with
all the copies in the previous snippet? Shouldn't they be moves? We have a move constructor
defined for `Foo`.

Well, it turns out due to exception-safe guarantees, C++ standard collections  will not use
non-exception-safe move constructors. So how do we fix this? Add `noexcept` to the move
constructor, like so:

``` {#function .cpp .numberLines startFrom="1"}
Foo(Foo&& f) noexcept {
  std::cout << "move constructor\n";
}
```

Then run the example without the `list.reserve(3)` again:

```
$ ./a.out
constructor2
---
constructor
move constructor
move constructor
---
constructor2
move constructor
move constructor
```

Congratulations, now you know more about `std::vector` than you wanted to know.
