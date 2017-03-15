#!/bin/env bash

# This script pulls the latest updates from upstream and
# installs pushes the updated files to the webserver

set -e

SITE_GIT_DIR=/home/daniel/dxuuu.xyz

cd $SITE_GIT_DIR
git pull origin master --ff-only
make clean
make
make install
