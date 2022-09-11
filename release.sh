#!/bin/bash
set -ex

if [[ -n $(git status --porcelain) ]]; then
  echo "Working tree is dirty. Refusing to continue."
  exit 1
fi

if ! [ "$(git rev-parse --abbrev-ref HEAD)" = "master" ]; then
  echo "Not on master branch. Refusing to continue."
  exit 1
fi

LATEST_TAG=$(git tag | grep -E "\d+\.\d+.\d+" | sort -rV | head -n1)
echo "${LATEST_TAG}"
NEXT_TAG=$(echo "${LATEST_TAG}" | awk -F. -v OFS=. '{$(NF-1) += 1 ; print}')
echo "${NEXT_TAG}"
git checkout --detach
sed -i -e "s/0.0.0/$NEXT_TAG/" src/git_perf/__version__.py
git add src/git_perf/__version__.py
git commit -m "Release version $NEXT_TAG"
git tag -a -m "$NEXT_TAG" "$NEXT_TAG"
git tag -f "latest"
git push -f origin "$NEXT_TAG" "latest"
git checkout master
