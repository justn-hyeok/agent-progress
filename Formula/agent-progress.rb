class AgentProgress < Formula
  desc "Passive terminal progress dashboard for coding agents"
  homepage "https://github.com/justn-hyeok/agent-progress"
  url "https://github.com/justn-hyeok/agent-progress/releases/download/v1.2.0/agent-progress-1.2.0-macos-arm64.tar.gz"
  sha256 "657bc6f1b941fd14d61ffb584fef6f0b6e01e0ab2cff6efa2842690289cb449d"

  depends_on arch: :arm64
  depends_on :macos

  def install
    bin.install "ap"
    doc.install "README.md", "OPERATIONS.md", "CHANGELOG.md", "THIRD_PARTY.md"
    pkgshare.install "theme-example.yaml"
  end

  test do
    ENV["HERDR_ENV"] = "0"
    assert_match version.to_s, shell_output("#{bin}/ap --version")
    system bin/"ap", "--file", "plan.md", "init", "--project", "Homebrew", "--goal", "Smoke test"
    assert_match "Smoke test", shell_output("#{bin}/ap --file plan.md show")
    system bin/"ap", "config", "set", "--preset", "forest", "--brightness", "1.2"
    system bin/"ap", "config", "init-yaml"
    assert_match "forest", (testpath/".agent-progress/ui.yaml").read
  end
end
