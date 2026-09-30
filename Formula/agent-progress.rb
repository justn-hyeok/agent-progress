class AgentProgress < Formula
  desc "Passive terminal progress dashboard for coding agents"
  homepage "https://github.com/justn-hyeok/agent-progress"
  url "https://github.com/justn-hyeok/agent-progress/releases/download/v1.2.6/agent-progress-1.2.6-macos-arm64.tar.gz"
  sha256 "f52e82a53f8a77766de23039637434596eb818f9221926322a398f31dd905aea"

  depends_on arch: :arm64
  depends_on :macos

  def install
    bin.install "ap"
    doc.install "README.md", "OPERATIONS.md", "CHANGELOG.md", "THIRD_PARTY.md"
    pkgshare.install "theme-example.yaml"
    pkgshare.install "skills"
  end

  def caveats
    <<~EOS
      진행 창 크기는 사용할 프로젝트 루트에서 설정할 수 있습니다:
        ap setup
      건너뛰면 기본 높이 30%를 사용합니다.
    EOS
  end

  test do
    ENV["HERDR_ENV"] = "0"
    assert_match version.to_s, shell_output("#{bin}/ap --version")
    system bin/"ap", "--file", "plan.md", "init", "--project", "Homebrew", "--goal", "Smoke test"
    assert_match "Smoke test", shell_output("#{bin}/ap --file plan.md show")
    system bin/"ap", "config", "set", "--preset", "forest", "--brightness", "1.2"
    system bin/"ap", "config", "set", "--pane-size", "40"
    assert_match '"pane_size_percent": 40', shell_output("#{bin}/ap config show")
    system bin/"ap", "config", "init-yaml"
    assert_match "forest", (testpath/".agent-progress/ui.yaml").read
    assert_path_exists pkgshare/"skills/ap/SKILL.md"
    assert_path_exists pkgshare/"skills/ap-connect/SKILL.md"
    assert_path_exists pkgshare/"skills/ap-theme/SKILL.md"
    assert_path_exists pkgshare/"skills/ap-recover/SKILL.md"
  end
end
