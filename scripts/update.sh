#!/usr/bin/env bash

# This script pulls the latest updates from upstream and
# installs pushes the updated files to the webserver

set -e

SITE_GIT_DIR="$HOME"/dxuuu.xyz

cd $SITE_GIT_DIR
git reset --hard HEAD
git pull origin master --ff-only
make install
