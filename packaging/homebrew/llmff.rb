class Llmff < Formula
  desc "FFmpeg-shaped command-line runner for LLM inference pipelines"
  homepage "https://github.com/syndicalt/llmff"
  version "0.1.2"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/syndicalt/llmff/releases/download/v0.1.2/llmff-0.1.2-aarch64-apple-darwin.tar.gz"
      sha256 "7b15b3d8510aaebe88c5d273c2b8f92e33e80d0b9a1e5a87492d1488116445ee"
    else
      url "https://github.com/syndicalt/llmff/releases/download/v0.1.2/llmff-0.1.2-x86_64-apple-darwin.tar.gz"
      sha256 "89780bd77f30584b06dfaf8b8179070fb87bdc497b4b3c1ca4f427eeb7dfe7ca"
    end
  end

  on_linux do
    url "https://github.com/syndicalt/llmff/releases/download/v0.1.2/llmff-0.1.2-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "48e290e689af48300af7ca9e3a53e2813a35d1b20c2228feae9c67d7412a1067"
  end

  def install
    bin.install "llmff"
  end

  test do
    assert_match "llmff 0.1.2", shell_output("#{bin}/llmff --version")
  end
end
