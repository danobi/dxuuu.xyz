% C++ patterns: static registration

Plugin architectures are useful: it's hard to predict future needs in the present.
Thus, it's often useful to punt as much business logic as possible to the future.
But how do we create a robust and scalable plugin architecture?

A naive approach is a large if-block. Suppose you have a configuration file where
the user specifies which plugin he or she wants by name:

#### config.toml
```
plugin=MyPlugin
```

The easiest way to support selecting plugins could look something like:

``` {#function .cpp .numberLines startFrom="1"}
if (plugin_name == "default") {
    return std::make_unique<Default>();
} else if (plugin_name == "myplugin") {
    return std::make_unique<MyPlugin>();
} else {
    return nullptr;
}

```

You could imagine this code exists in the "core" codebase. However, this is
not scalable. Every time a developer authors a plugin (or renames an existing)
plugin, the core implementation needs to change. It would be far better if we
could make the addition, change, or removal of plugins generic.

The static registration pattern accomplishes this. We take advantage of the
fact that static variables are initialized _before_ `main()` is reached.
Furthermore, static variables may call other static methods. [citation?]
Consider a plugin "registry" implemented like so:

#### Registry.h
``` {#function .cpp .numberLines startFrom="1"}
#define REGISTER_PLUGIN(plugin_name, create_func) \
    bool plugin_name ## _entry = PluginRegistry<Plugin>::add(#plugin_name, (create_func))

template <typename T>
class PluginRegistry {
  public:
    typedef std::function<T*()> FactoryFunction;
    typedef std::unordered_map<std::string, FactoryFunction> FactoryMap;

    static bool add(const std::string& name, FactoryFunction fac) {
      auto map = getFactoryMap();
      if (map.find(name) != map.end()) {
        return false;
      }

      getFactoryMap()[name] = fac;
      return true;
    }

    static T* create(const std::string& name) {
      auto map = getFactoryMap();
      if (map.find(name) == map.end()) {
        return nullptr;
      }

      return map[name]();
    }

  private:
    // Use Meyer's singleton to prevent SIOF
    static FactoryMap& getFactoryMap() {
      static FactoryMap map;
      return map;
    }
};

```

Notice how `PluginRegistry` is completely generic. It can hold factory methods
for any type. On line 1, we define a macro that specializes `PluginRegistry`
for a class `Plugin`, which we'll pretend is trivial.

In this manner, when a developer authors a plugin, registration is trivial.

#### RealPlugin.h

``` {#function .cpp .numberLines startFrom="1"}
class RealPlugin : public Plugin {
  public:
    void doWork() override {
      std::cout << "I\'m doing real work!\n";
    };

    static Plugin* create() {
      return new RealPlugin();
    }
};

REGISTER_PLUGIN(RealPlugin, RealPlugin::create);
```

And in `Main.cpp`, we can instantiate `RealPlugin` like this:

``` {#function .cpp .numberLines startFrom="1"}
p = PluginRegistry<Plugin>::create("RealPlugin");
p->doWork();
```

And so, our nasty if block turns into:

``` {#function .cpp .numberLines startFrom="1"}
return std::unique_ptr<Plugin>(PluginRegistry<Plugin>::create(plugin_name));
```

## Note on linking

Particularly astute readers might wonder: will the linker garbage collect my
static plugin variables (ie ` bool *_entry = ...`)? The answer is maybe,
depending on how you're building your program. Because the "core" program does
not "touch" any symbols in plugin translation units, some linkers assume that
it is safe to garbage collect the plugin TU. In most cases, this is safe and ok
and a worthy optimization.  However in our case, this is unwanted.

The solution is to pass to the linker a "-lwhole-achive" [citation?] flag.
