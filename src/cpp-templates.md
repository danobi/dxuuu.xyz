% Playing with C++ templates

Last week I found myself quite sick. Rather than wallow in it,
I played around with C++ templates. Until now, I've only parameterized classes into generic
functions. As a daily user of [folly](https://github.com/facebook/folly),
I've spent quite some time puzzling over C++ template magic. It's time I got my feet wet.

### Simple example

We want a generic function to call a standard interface:

``` {#function .cpp .numberLines startFrom="1"}
#include <iostream>

struct A {
    int a() {
        return 1;
    }
};

struct B {
    int a() {
        return 2;
    }
};

template <typename T>
void foo() {
    T t;
    std::cout << t.a() << "\n";
}

int main() {
    foo<A>();
    foo<B>();
}
```

When compiled and run:
```
$ g++ typename.cpp
$ ./a.out
1
2
```

Note that we _may_ choose to use old-style `class` instead of `typename`
on line 15. However, for clarity, `class` should be avoided. Trivia:
Bjarne originally reused the `class` keyword in template declarations to avoid another reserved
keyword.

### Integers inside templates

Now another example. Did you know templates declarations can specify more than class types?
In fact, according to the [cppreference][0], we can use:

* std::nullptr_t (since C++11)
* integral type
* lvalue reference type (to object or to function)
* pointer type (to object or to function)
* pointer to member type (to member object or to member function)
* enumeration type

Let's play around with an integer type inside a template:

``` {#function .cpp .numberLines startFrom="1"}
#include <iostream>

template <int N = 10>
void foo() {
    std::cout << N << "\n";
}

int main() {
    foo<1>();
    foo<2>();
    foo();
}
```

When compiled and run:
```
$ g++ int.cpp
$ ./a.out
1
2
10
```

One can see how this might be useful to flexibly declare static array sizes at compile time (to avoid [variable-length arrays][2], whose
ugliness is [described in last week's LWN][3]).

### Template metaprogramming

Now that we've covered some template basics, let's play around with
template metaprogramming. Succinctly, template metaprogramming
is the ability to do fancy compile time things. For example, a library
developer can omit certain features from being compiled
if an application does not use them. One way to do this
is through [`std::enable_if`][1]. In folly, [the futures library][4]
overloads `onError(F&& func)` to return different return types.
Folly prevents binary bloat by omitting unused overloads from compilation.
The tradeoff in this case is a slightly longer compile times.

We will do something far less complicated. We will play with [`std::decay`][5],
a C++ standard library function that "decays" types. For example, the type `Foo&` and `Foo`
should, in many cases, be considered the same. However, a C++ compiler cannot
safely assume that is always the case, especially for functions like [`std::is_same`][6].

In the spirit of flexibility, the C++ committee provides a mechanism to "decay"
types into "base" types. Hopefully that makes more sense than the official
documentation:

    Applies lvalue-to-rvalue, array-to-pointer, and function-to-pointer implicit conversions to the type T, removes cv-qualifiers, and defines the resulting type as the member typedef type.


``` {#function .cpp .numberLines startFrom="1"}
#include <iostream>
#include <type_traits>

struct A {};
struct B : A {};

int main() {
    std::cout << std::boolalpha
        << std::is_same<A, B>::value << '\n'
        << std::is_same<A, A>::value << '\n'
        << std::is_same<A, std::decay<A*>::type>::value << '\n'
        << std::is_same<A, std::decay<A&>::type>::value << '\n';
}
```

While seemingly complicated, this snippet is quite simple. We perform 4
checks on lines 9-12: if

* types `A` and `B` are equivalent (one should hope not)
* types `A` and `A` are equivalent (one should hope so)
* if `A*` decays into `A`
* if `A&` decays into `A`

When compiled and run:
```
$ g++ decay.cpp
$ ./a.out
false
true
false
true
```

Hopefully checks 1 and 2 are not surprising. Checks 3 and 4 deserve a brief
explanation:

Check 3 shows that `A*` does not decay into `A`. This makes sense because at
runtime, one cannot perform the same operations on `A*` as on `A`. For instance,
if `A` were a class, to call a method on type `A`, one does `A.foo()`. One
cannot do the same with `A*`. `A*` needs to be dereferenced first.

Check 4 shows that `A&` decays into `A`. This makes sense because at runtime,
most non-metaprogramming operations work the same.

[0]: http://en.cppreference.com/w/cpp/language/template_parameters
[1]: http://en.cppreference.com/w/cpp/types/enable_if
[2]: https://en.wikipedia.org/wiki/Variable-length_array
[3]: https://lwn.net/Articles/749064/
[4]: https://github.com/facebook/folly/blob/master/folly/futures/Future.h#L629-L674
[5]: http://en.cppreference.com/w/cpp/types/decay
[6]: http://en.cppreference.com/w/cpp/types/is_same
