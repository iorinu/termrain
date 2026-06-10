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

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/iorinu/termrain/releases/download/v#{version}/termrain-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_DARWIN_SHA256"
    else
      url "https://github.com/iorinu/termrain/releases/download/v#{version}/termrain-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_DARWIN_SHA256"
    end
  end

  on_linux do
    url "https://github.com/iorinu/termrain/releases/download/v#{version}/termrain-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "REPLACE_WITH_X86_64_LINUX_SHA256"
  end

  def install
    bin.install "termrain"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/termrain --version")
  end
end
