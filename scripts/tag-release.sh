#!/bin/sh
# This script sets things up to make a release,
# creating a tag based on the current commit.
# All numbers, so that Home Assistant can order them: with the hash in the
# version it could not tell two releases of the same day apart.
TAG_NAME=${TAG_NAME:-$(TZ=UTC0 git show -s --format=%cd --date=format-local:%Y.%m.%d.%H%M)}
git tag $TAG_NAME
./scripts/apply-tag.sh
docker run -t -v "$(pwd)":/app/ "ghcr.io/orhun/git-cliff/git-cliff:${TAG:-latest}" -o addon/CHANGELOG.md -c scripts/cliff.toml
git add addon/config.yaml addon/CHANGELOG.md
git commit -m "Tag $TAG_NAME"
