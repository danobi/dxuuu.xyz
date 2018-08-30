#include "Plugin.h"
#include "RealPlugin.h"
#include "SplitPlugin.h"
#include "Registry.h"

REGISTER_PLUGIN(Plugin, Plugin::create);

int main() {
  Plugin* p = PluginRegistry<Plugin>::create("Plugin");
  if (!p) {
    std::cout << "null\n";
    return 1;
  }
  p->doWork();

  p = PluginRegistry<Plugin>::create("RealPlugin");
  if (!p) {
    std::cout << "null\n";
    return 1;
  }
  p->doWork();

  p = PluginRegistry<Plugin>::create("SplitPlugin");
  if (!p) {
    std::cout << "null\n";
    return 1;
  }
  p->doWork();

  return 0;
}
