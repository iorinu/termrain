"""
リリース時に homebrew-tap の termrain.rb を自動更新するスクリプト。
環境変数:
  VERSION             : "v0.2.2" 形式のタグ名
  HOMEBREW_TAP_TOKEN  : homebrew-tap への書き込み権限を持つ PAT
"""

import base64
import json
import os
import sys
import urllib.request
from pathlib import Path

VERSION = os.environ["VERSION"]
TOKEN = os.environ["HOMEBREW_TAP_TOKEN"]
VER = VERSION.lstrip("v")

ARTIFACTS = Path("artifacts")


def read_sha256(target: str) -> str:
    path = ARTIFACTS / target / f"termrain-{VERSION}-{target}.tar.gz.sha256"
    return path.read_text().split()[0]


sha_arm = read_sha256("aarch64-apple-darwin")
sha_intel = read_sha256("x86_64-apple-darwin")
sha_linux = read_sha256("x86_64-unknown-linux-gnu")

FORMULA = f"""\
class Termrain < Formula
  desc "Terminal weather forecast and rain radar TUI (JMA + Open-Meteo, Kitty graphics)"
  homepage "https://github.com/iorinu/termrain"
  version "{VER}"
  license "MIT"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/iorinu/termrain/releases/download/{VERSION}/termrain-{VERSION}-aarch64-apple-darwin.tar.gz"
      sha256 "{sha_arm}"
    end
    if Hardware::CPU.intel?
      url "https://github.com/iorinu/termrain/releases/download/{VERSION}/termrain-{VERSION}-x86_64-apple-darwin.tar.gz"
      sha256 "{sha_intel}"
    end
  end
  if OS.linux? && Hardware::CPU.intel?
    url "https://github.com/iorinu/termrain/releases/download/{VERSION}/termrain-{VERSION}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "{sha_linux}"
  end

  def install
    bin.install "termrain"

    doc_files = Dir["README.*", "readme.*", "LICENSE", "LICENSE.*", "CHANGELOG.*"]
    leftover = Dir["*"] - doc_files - ["termrain"]
    pkgshare.install(*leftover) unless leftover.empty?
  end

  test do
    assert_match version.to_s, shell_output("#{{bin}}/termrain --version")
  end
end
"""

API_URL = "https://api.github.com/repos/iorinu/homebrew-tap/contents/Formula/termrain.rb"
HEADERS = {"Authorization": f"token {TOKEN}", "Content-Type": "application/json"}


def api(method: str, data: dict | None = None):
    body = json.dumps(data).encode() if data else None
    req = urllib.request.Request(API_URL, data=body, headers=HEADERS, method=method)
    return json.loads(urllib.request.urlopen(req).read())


file_sha = api("GET")["sha"]
result = api("PUT", {
    "message": f"chore: bump termrain to {VERSION}",
    "content": base64.b64encode(FORMULA.encode()).decode(),
    "sha": file_sha,
})
print("Updated:", result["commit"]["html_url"])
