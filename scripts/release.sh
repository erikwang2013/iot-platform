#!/usr/bin/env bash
# 推送后自动发布：取最新版本 → 增量（默认 minor，可传 major/patch）→ 打标签推送 → 建 GitHub Release（增量 changelog）
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
bump="${1:-minor}"

latest=$(git tag --sort=-v:refname | head -1 || true)
if [[ -z "$latest" ]]; then latest="v0.0.0"; fi

# 无新提交则跳过（增量语义）
if git rev-parse -q --verify "$latest" >/dev/null && [[ -z "$(git log --oneline "$latest..HEAD")" ]]; then
    echo "无新提交（$latest 已是最新），跳过发布"
    exit 0
fi

IFS=. read -r major minor patch <<< "${latest#v}"
case "$bump" in
    major) major=$((major + 1)); minor=0; patch=0 ;;
    minor) minor=$((minor + 1)); patch=0 ;;
    patch) patch=$((patch + 1)) ;;
    *) echo "bump 须为 major|minor|patch（实际: $bump）" >&2; exit 1 ;;
esac
tag="v${major}.${minor}.${patch}"

notes=$(git log --oneline "$latest..HEAD" 2>/dev/null || echo "初始版本")
git tag -a "$tag" -m "IoT 平台 $tag"
git push origin "$tag"
if command -v gh >/dev/null && gh auth status >/dev/null 2>&1; then
    gh release create "$tag" --title "IoT 平台 $tag" --notes "$notes"
else
    echo "gh 不可用或未认证：标签 $tag 已推送，请手动创建 Release"
fi
echo "发布完成：$latest -> $tag"
