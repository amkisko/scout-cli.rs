# frozen_string_literal: true

# ScoutAPM CLI - Homebrew Formula
# Install: brew install amkisko/tap/scout-cli
# Or: brew tap amkisko/tap && brew install scout-cli
class ScoutCli < Formula
  desc "ScoutAPM CLI — query apps, endpoints, traces, metrics, and errors"
  homepage "https://github.com/amkisko/scout-cli.rs"
  url "https://github.com/amkisko/scout-cli.rs/archive/refs/tags/v0.1.0.tar.gz"
  # Fill before release: shasum -a 256 <(curl -sL https://github.com/amkisko/scout-cli.rs/archive/refs/tags/vX.Y.Z.tar.gz)
  sha256 ""
  license "MIT"
  head "https://github.com/amkisko/scout-cli.rs.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "scout")
  end

  test do
    assert_match "scout #{version}", shell_output("#{bin}/scout version")
  end
end
