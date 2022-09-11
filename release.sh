#!/bin/bash
set -ex

if ! git diff-index --quiet --cached HEAD --; then
  echo "There are staged changes. Refusing to continue."
  exit 1
fi

LATEST_TAG=$(git tag | grep -E "\d+\.\d+.\d+" | sort -rV | head -n1)
echo "${LATEST_TAG}"
NEXT_TAG=$(echo "${LATEST_TAG}" | awk -F. -v OFS=. '{$(NF-1) += 1 ; print}')
echo "${NEXT_TAG}"
RETURN_TO=$(git rev-parse --abbrev-ref HEAD)
git checkout --detach
sed -i -e "s/<<VERSION>>/$NEXT_TAG/" src/git_perf/__version__.py
git add src/git_perf/__version__.py
git commit -m "Release version $NEXT_TAG"
git tag -a -m "$NEXT_TAG" "$NEXT_TAG"
git tag -f "latest"
git checkout "$RETURN_TO"
