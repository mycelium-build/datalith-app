#!/usr/bin/env bash

set -euo pipefail

sudo apt-get update
sudo apt-get install --yes --no-install-recommends \
  clang \
  cmake \
  libasound2-dev \
  libfontconfig1-dev \
  libgl1-mesa-dev \
  libwayland-dev \
  libx11-xcb-dev \
  libxkbcommon-dev \
  libxkbcommon-x11-dev \
  libxcursor-dev \
  libxi-dev \
  libxrandr-dev \
  pkg-config
