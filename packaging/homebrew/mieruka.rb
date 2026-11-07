class Mieruka < Formula
  desc "See which instruction files your coding agents load, and what changed in them"
  homepage "https://github.com/ayokiru/mieruka"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/ayokiru/mieruka/releases/download/v{{VERSION}}/mieruka-aarch64-apple-darwin.tar.gz"
      sha256 "{{SHA256_MACOS_ARM}}"
    end
    on_intel do
      url "https://github.com/ayokiru/mieruka/releases/download/v{{VERSION}}/mieruka-x86_64-apple-darwin.tar.gz"
      sha256 "{{SHA256_MACOS_INTEL}}"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/ayokiru/mieruka/releases/download/v{{VERSION}}/mieruka-x86_64-unknown-linux-musl.tar.gz"
      sha256 "{{SHA256_LINUX}}"
    end
  end

  def install
    bin.install "mieruka"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mieruka --version")
  end
end
