class AgentProgress < Formula
  desc "Passive terminal progress dashboard for coding agents"
  homepage "https://github.com/justn-hyeok/agent-progress"
  url "https://github.com/justn-hyeok/agent-progress/releases/download/v1.2.1/agent-progress-1.2.1-macos-arm64.tar.gz"
  sha256 "d4fedf795f36f6fd78c3e367dcc00caf2c3a1df51832b16becfeb2160a4588ef"

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
