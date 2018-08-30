#pragma once

#include <string>
#include <iostream>

class Plugin {
  public:
    virtual void doWork() {
      std::cout << "I\'m the base plugin!\n";
    };

    static Plugin* create() {
      return new Plugin();
    }
};
