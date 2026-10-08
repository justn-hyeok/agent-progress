class AgentProgress < Formula
  desc "Terminal progress pane for coding agent plans"
  homepage "https://github.com/justn-hyeok/agent-progress"
  url "https://github.com/justn-hyeok/agent-progress/releases/download/v3.3.0/agent-progress-3.3.0-macos-arm64.tar.gz"
  sha256 "a09d892ae8dddc38bcc616a67fd3c62de6c75a604831681b7d2c1cfe3578286c"

  depends_on arch: :arm64
  depends_on :macos

  def install
    bin.install "ap"
    doc.install "README.md", "OPERATIONS.md", "CHANGELOG.md", "THIRD_PARTY.md"
    pkgshare.install "skills"
  end

  def caveats
    <<~EOS
      To let coding agents (Claude Code, Codex, Cursor, …) use ap on their own:
        ap skill install
    EOS
  end

  test do
    ENV["HERDR_ENV"] = "0"
    ENV["AP_AUTO_OPEN"] = "0"
    assert_match version.to_s, shell_output("#{bin}/ap --version")
    system bin/"ap", "goal", "Smoke test"
    system bin/"ap", "add", "one", "two"
    system bin/"ap", "done", "1"
    assert_match "Smoke test · 1/2 (50%)", shell_output("#{bin}/ap status")
    assert_path_exists pkgshare/"skills/ap/SKILL.md"
  end
end
