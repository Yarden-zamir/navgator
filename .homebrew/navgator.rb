# typed: false
# frozen_string_literal: true

class Navgator < Formula
  desc "Rust TUI project navigator with Git worktree and preview support"
  homepage "https://github.com/Yarden-zamir/navgator"
  url "{{URL}}"
  sha256 "{{SHA256}}"
  license "MIT"
  head "https://github.com/Yarden-zamir/navgator.git", branch: "main"

  depends_on "rust" => :build
  depends_on "erdtree"
  depends_on "gh"

  def install
    system "cargo", "install", *std_cargo_args
    pkgshare.install "scripts/navgator.zsh"
  end

  def caveats
    <<~EOS
      To activate the zsh navigator widget, add this to your .zshrc:
        source #{opt_pkgshare}/navgator.zsh

      Then bind it, for example:
        bindkey '^T' navigate
    EOS
  end

  test do
    assert_match "Usage:", shell_output("#{bin}/navgator --help 2>&1")
  end
end
