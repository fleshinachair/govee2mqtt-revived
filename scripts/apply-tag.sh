#!/bin/sh
# This script updates the version number for
# the addon based on the current commit timestamp
# All numbers, so that Home Assistant can order them: with the hash in the
# version it could not tell two releases of the same day apart.
TAG_NAME=${TAG_NAME:-$(TZ=UTC0 git show -s --format=%cd --date=format-local:%Y.%m.%d.%H%M)}
sed -i "s/version:.*/version: \"$TAG_NAME\"/" addon/config.yaml
