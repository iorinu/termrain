#!/usr/bin/env bash
# Codex レビューを push 前に実行する、個人用の Git hook をインストールする。
# .git/hooks は追跡対象外なので、各開発者が明示的に実行する。
set -euo pipefail

hook_path="$(git rev-parse --git-path hooks/pre-push)"

if [[ -e "$hook_path" ]]; then
  echo "error: $hook_path は既に存在します。既存 hook を上書きしないため中止しました。" >&2
  exit 1
fi

mkdir -p "$(dirname "$hook_path")"
cat >"$hook_path" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail

# 緊急時だけ明示的に bypass できる。通常の push ではレビューを必ず実行する。
if [[ "${TERMRAIN_SKIP_CODEX_REVIEW:-}" == "1" ]]; then
  echo "Skipping Codex review because TERMRAIN_SKIP_CODEX_REVIEW=1."
  exit 0
fi

repo_root="$(git rev-parse --show-toplevel)"
zero_sha="0000000000000000000000000000000000000000"

# pre-push の stdin は local ref/SHA と remote ref/SHA の組を返す。
# 作業ツリーではなく、実際に送信される commit range をレビューする。
while read -r local_ref local_sha remote_ref remote_sha; do
  if [[ "$local_sha" == "$zero_sha" ]]; then
    # branch の削除ではレビュー対象となる新しいコミットがない。
    continue
  fi

  if [[ "$remote_sha" == "$zero_sha" ]]; then
    # 新規 branch は main との merge base 以降だけをレビューする。
    base_sha="$(git merge-base "$local_sha" origin/main)"
  else
    base_sha="$remote_sha"
  fi

  "$repo_root/scripts/review-with-codex.sh" "$base_sha" "$local_sha"
done
HOOK
chmod +x "$hook_path"

echo "Installed $hook_path"
echo "通常の push 前に Codex review が走ります。緊急時だけ TERMRAIN_SKIP_CODEX_REVIEW=1 git push で bypass できます。"
