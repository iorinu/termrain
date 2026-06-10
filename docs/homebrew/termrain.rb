# Homebrew Formula for termrain
#
# 配置先: iorinu/homebrew-tap リポジトリの Formula/termrain.rb
# 利用者は次の手順でインストールする:
#   brew tap iorinu/tap
#   brew install termrain
#
# このファイルはリポジトリ内に参考として置いているだけ。実際の Formula は
# iorinu/homebrew-tap で管理する。`url` と `sha256` は v0.1.0 リリース後に
# 各プラットフォームのアーカイブから差し替える。

class Termrain < Formula
  desc "Terminal weather forecast and rain radar TUI (JMA + Open-Meteo, Kitty graphics)"
  homepage "https://github.com/iorinu/termrain"
  version "0.1.0"
  license "MIT"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/iorinu/termrain/releases/download/v0.1.0/termrain-v0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_DARWIN_SHA256"
    end
    if Hardware::CPU.intel?
      url "https://github.com/iorinu/termrain/releases/download/v0.1.0/termrain-v0.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_DARWIN_SHA256"
    end
  end
  if OS.linux? && Hardware::CPU.intel?
    url "https://github.com/iorinu/termrain/releases/download/v0.1.0/termrain-v0.1.0-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "REPLACE_WITH_X86_64_LINUX_SHA256"
  end

  def install
    bin.install "termrain"

    # README / LICENSE はアーカイブに同梱しているので Homebrew に任せる
    doc_files = Dir["README.*", "readme.*", "LICENSE", "LICENSE.*", "CHANGELOG.*"]
    leftover = Dir["*"] - doc_files - ["termrain"]
    pkgshare.install(*leftover) unless leftover.empty?
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/termrain --version")
  end
end
