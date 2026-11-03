class AgentProgress < Formula
  desc "Terminal progress pane for coding agent plans"
  homepage "https://github.com/justn-hyeok/agent-progress"
  url "https://github.com/justn-hyeok/agent-progress/releases/download/v3.5.0/agent-progress-3.5.0-macos-arm64.tar.gz"
  sha256 "e9d33315d75c1d2ea8afba98e4005d3cd20294d4fc004f94c087c6dfa6f92900"

  depends_on arch: :arm64
  depends_on :macos

  def install
    bin.install "ap"
    doc.install "README.md", "OPERATIONS.md", "CHANGELOG.md", "THIRD_PARTY.md"
    pkgshare.install "skills"
  end

  def caveats
    <<~EOS
      To let coding agents (Claude Code, Codex, Cursor, …) record with ap on their own:
        ap skill install
      Then run `ap open` in an agent's pane to watch its progress.
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
