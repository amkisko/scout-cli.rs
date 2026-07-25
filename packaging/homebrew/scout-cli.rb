# frozen_string_literal: true

# ScoutAPM CLI - Homebrew Formula
# Install: brew install amkisko/tap/scout-cli
# Or: brew tap amkisko/tap && brew install scout-cli
class ScoutCli < Formula
  desc "ScoutAPM CLI — query apps, endpoints, traces, metrics, and errors"
  homepage "https://github.com/amkisko/scout-cli.rs"
  url "https://github.com/amkisko/scout-cli.rs/archive/refs/tags/v0.4.0.tar.gz"
  # Fill before release: shasum -a 256 <(curl -sL https://github.com/amkisko/scout-cli.rs/archive/refs/tags/vX.Y.Z.tar.gz)
  sha256 "1cf42adadd3fd9087967bd4c4dfae4ac855fc4745d746bb947883f128e5d3a5c"
  license "MIT"
  head "https://github.com/amkisko/scout-cli.rs.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "scout")
    bash_completion.install shell_output("#{bin}/scout completions bash"), "scout"
    zsh_completion.install shell_output("#{bin}/scout completions zsh"), "_scout"
    fish_completion.install shell_output("#{bin}/scout completions fish"), "scout.fish"
    man1.install shell_output("#{bin}/scout man"), "scout.1"
  end

  test do
    assert_match "scout #{version}", shell_output("#{bin}/scout version")
  end
end
