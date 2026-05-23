class Llmff < Formula
  desc "FFmpeg-shaped command-line runner for LLM inference pipelines"
  homepage "https://github.com/syndicalt/llmff"
  version "0.1.3"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/syndicalt/llmff/releases/download/v0.1.3/llmff-0.1.3-aarch64-apple-darwin.tar.gz"
      sha256 "d3ceb2cb6714ad27e18c8ab52b2f5265fc7424615a5a6ceb38ad95cff63d3e4d"
    else
      url "https://github.com/syndicalt/llmff/releases/download/v0.1.3/llmff-0.1.3-x86_64-apple-darwin.tar.gz"
      sha256 "a97900222ac7a59c550ee4648998824c54a1a70d195173150ab4db6bb7c663aa"
    end
  end

  on_linux do
    url "https://github.com/syndicalt/llmff/releases/download/v0.1.3/llmff-0.1.3-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "e83fe32cf6bc88acc426acadc893e9015a4d58bc47ac8726a21b54b685b7950c"
  end

  def install
    bin.install "llmff"
  end

  test do
    assert_match "llmff 0.1.3", shell_output("#{bin}/llmff --version")
  end
end
