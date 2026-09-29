class AgentProgress < Formula
  desc "Passive terminal progress dashboard for coding agents"
  homepage "https://github.com/justn-hyeok/agent-progress"
  url "https://github.com/justn-hyeok/agent-progress/releases/download/v1.2.5/agent-progress-1.2.5-macos-arm64.tar.gz"
  sha256 "01af4d0d6327aeda74f41aed53e0713c27d80fd98d05a3451abcd8ac0cc2ed68"

  depends_on arch: :arm64
  depends_on :macos

  def install
    bin.install "ap"
    doc.install "README.md", "OPERATIONS.md", "CHANGELOG.md", "THIRD_PARTY.md"
    pkgshare.install "theme-example.yaml"
    pkgshare.install "skills"
  end

  test do
    ENV["HERDR_ENV"] = "0"
    assert_match version.to_s, shell_output("#{bin}/ap --version")
    system bin/"ap", "--file", "plan.md", "init", "--project", "Homebrew", "--goal", "Smoke test"
    assert_match "Smoke test", shell_output("#{bin}/ap --file plan.md show")
    system bin/"ap", "config", "set", "--preset", "forest", "--brightness", "1.2"
    system bin/"ap", "config", "init-yaml"
    assert_match "forest", (testpath/".agent-progress/ui.yaml").read
    assert_path_exists pkgshare/"skills/ap/SKILL.md"
    assert_path_exists pkgshare/"skills/ap-connect/SKILL.md"
    assert_path_exists pkgshare/"skills/ap-theme/SKILL.md"
    assert_path_exists pkgshare/"skills/ap-recover/SKILL.md"
  end
end
