#pragma once

#include <string>
#include <iostream>

#include "Plugin.h"
#include "Registry.h"

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
