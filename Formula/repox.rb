# typed: false
# frozen_string_literal: true

# Homebrew Formula for repOx (WVDYC/repOx)
class Repox < Formula
  desc "Blazing-fast repository-to-prompt CLI & TUI packer for LLM context windows"
  homepage "https://github.com/WVDYC/repOx"
  version "0.1.0"
  license any_of: ["MIT", "Apache-2.0"]
  head "https://github.com/WVDYC/repOx.git", branch: "main"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/WVDYC/repOx/releases/download/v0.1.0/repox-v0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # Updated on release tag
    else
      url "https://github.com/WVDYC/repOx/releases/download/v0.1.0/repox-v0.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # Updated on release tag
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/WVDYC/repOx/releases/download/v0.1.0/repox-v0.1.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # Updated on release tag
    else
      url "https://github.com/WVDYC/repOx/releases/download/v0.1.0/repox-v0.1.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # Updated on release tag
    end
  end

  def install
    bin.install "repox"
    bash_completion.install "completions/repox.bash" => "repox"
    zsh_completion.install "completions/_repox" => "_repox"
    fish_completion.install "completions/repox.fish"
  end

  def caveats
    <<~EOS
      ⚡ repOx is now installed!
      
      Quickstart:
        repox -c             # Copy repo to clipboard as Claude-optimized XML
        repox -i -p claude   # Interactive TUI mode with real-time token budgeting
    EOS
  end

  test do
    assert_match "repox #{version}", shell_output("#{bin}/repox --version")
    
    # Test directory packaging
    testpath.install_symlink testpath
    (testpath/"test.txt").write("Hello LLM prompt packer")
    output = shell_output("#{bin}/repox #{testpath} -f md")
    assert_match "Hello LLM prompt packer", output
  end
end
