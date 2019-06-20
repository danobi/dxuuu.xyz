% Comparing interned strings

C/C++ has this interesting property where if you define a string literal, the
compiler will make sure to only keep one copy of it. This makes sense as it
reduces how much static storage is required.

For example, if you have:

```
const char* s = "asdf";
const char* s2 = "asdf";
```

the compiler will make sure "asdf" is stored once in the resulting binary.

This opens up an interesting property where you can compare string pointers as
if you were calling `strcmp`. For example:

``` {#function .cpp .numberLines startFrom="1"}
#include <iostream>

constexpr auto s1 = "string one";
constexpr auto s2 = "string two";
constexpr auto s1_copy = "string one";

int main() {
  if (s1 == s2) {
    std::cout << "s1 == s2" << std::endl;
  } else {
    std::cout << "s1 != s2" << std::endl;
  }

  if (s1 == s1_copy) {
    std::cout << "s1 == s1_copy" << std::endl;
  } else {
    std::cout << "s1 != s1_copy" << std::endl;
  }

  if (s1 == "string one") {
    std::cout << "s1 == string one" << std::endl;
  } else {
    std::cout << "s1 != string one" << std::endl;
  }

  return 0;
}
```

outputs:

```
$ ./interned_string
s1 != s2
s1 == s1_copy
s1 == string one
```

### Note

According to cppreference:

> The compiler is allowed, but not required, to combine storage for equal or
> overlapping string literals. That means that identical string literals may or
> may not compare equal when compared by pointer.

That being said, no sane compiler would omit this.
