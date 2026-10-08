mod herdr;
mod pane;
mod skill;
mod store;
mod tmux;
mod view;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use pane::{Decision, PaneState, Panes};
use std::path::PathBuf;
use store::{Plan, State, Store, Viewer};

/// 에이전트 작업 계획을 기록하고 별도 창에 보여줍니다. 체크율은 완료 보고 비율이며 제품 완성도가 아닙니다.
#[derive(Parser)]
#[command(name = "ap", version)]
struct Cli {
    /// 계획 이름. 기본값은 AP_PLAN, Herdr/tmux pane, 그 외에는 프로젝트 기본 계획입니다
    #[arg(long, global = true)]
    plan: Option<String>,
    /// 이번 명령에서 진행 창을 자동으로 열지 않습니다 (AP_AUTO_OPEN=0과 같음)
    #[arg(long, global = true)]
    no_view: bool,
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// 목표를 설정합니다
    Goal { text: String },
    /// 항목을 추가합니다. 번호는 한 번 정해지면 바뀌지 않습니다
    Add {
        #[arg(required = true)]
        titles: Vec<String>,
    },
    /// 항목을 진행 중으로 표시합니다 (번호 또는 정확한 제목)
    Start { item: String },
    /// 항목을 완료로 표시합니다
    Done {
        #[arg(required = true)]
        items: Vec<String>,
    },
    /// 항목을 막힘으로 표시합니다
    Block {
        item: String,
        reason: String,
        /// 누가 풀어야 하는지 (예: user, external)
        #[arg(long)]
        needs: Option<String>,
    },
    /// 항목을 취소합니다. 진행률 분모에서 빠집니다
    Cancel {
        item: String,
        reason: Option<String>,
    },
    /// 항목을 다시 예정으로 되돌립니다
    Todo { item: String },
    /// 잘못 추가한 항목을 지웁니다. 다른 항목 번호는 바뀌지 않습니다
    Rm { item: String },
    /// 현재 계획을 보관하고 새로 시작합니다
    New,
    /// 현재 계획을 출력합니다
    Status {
        #[arg(long)]
        json: bool,
    },
    /// 변경 이력을 출력합니다
    History,
    /// 진행 창을 현재 터미널에서 실행합니다
    View {
        /// 계획 파일을 직접 지정합니다
        #[arg(long)]
        file: Option<PathBuf>,
        /// 한 번만 출력하고 종료합니다
        #[arg(long)]
        once: bool,
        /// 자동으로 연 진행 창의 식별자 (내부용)
        #[arg(long, hide = true)]
        instance: Option<String>,
        /// 진행 창이 따라갈 pane 상태 파일 (내부용)
        #[arg(long, hide = true)]
        pane_state: Option<PathBuf>,
    },
    /// 현재 pane 아래에 진행 창을 엽니다 (자동 열기 억제 해제)
    Open,
    /// 이 계획의 진행 창을 닫고 자동 열기를 멈춥니다
    Close,
    /// 에이전트가 ap를 스스로 쓰도록 스킬과 지침을 설치·제거합니다
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
}

#[derive(Subcommand)]
enum SkillAction {
    /// ~/.agents/skills/ap 설치, 하네스 스킬 폴더 연결, Claude Code 지침 추가 (기존 파일은 백업)
    Install {
        /// 바꾸지 않고 할 일만 보여줍니다
        #[arg(long)]
        dry_run: bool,
    },
    /// ap가 설치한 스킬, 연결, 지침만 제거합니다
    Remove {
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone, Copy)]
enum Host {
    Herdr,
    Tmux,
    None,
}

struct Ctx {
    key: String,
    store: Store,
    root: PathBuf,
    host: Host,
    source_pane: Option<String>,
    /// Terminal identity that must match the stored plan for pane-keyed plans.
    terminal: Option<String>,
    /// The source pane's viewer state; one viewer per pane across all plans.
    panes: Option<Panes>,
}

fn context(plan: Option<String>) -> Result<Ctx> {
    let cwd = std::env::current_dir()?;
    let root = store::project_root(&cwd);
    let codex_thread = std::env::var("CODEX_THREAD_ID")
        .ok()
        .filter(|s| !s.is_empty());
    // HERDR_PANE_ID is inherited, not proven: Codex's shared daemon carries the ID of
    // whichever pane started it. Trust it only when that pane's process is our ancestor.
    let herdr_pane = std::env::var("HERDR_PANE_ID")
        .ok()
        .filter(|p| herdr::inside() && herdr::is_ancestor_pane(p));
    // Codex's daemon hides the pane; find the one Codex pane titled with this thread's name.
    let codex_pane = match (&herdr_pane, &codex_thread) {
        (None, Some(thread)) if herdr::inside() => herdr::codex_pane_for_thread(thread, &root),
        _ => None,
    };
    // Cline runs commands from a daemon without HERDR_* or any session identity; use the
    // one Cline pane working in this project, if there is exactly one.
    let cline_daemon = std::env::var_os("CLINE_WRAPPER_PATH").is_some() && !herdr::inside();
    let cline_pane = (herdr_pane.is_none() && codex_pane.is_none() && cline_daemon)
        .then(|| herdr::unique_agent_pane("cline", &root))
        .flatten();
    let tmux_pane = std::env::var("TMUX_PANE").ok().filter(|p| !p.is_empty());
    let (host, source_pane) = match (herdr_pane.or(codex_pane.clone()).or(cline_pane), tmux_pane) {
        (Some(p), _) => (Host::Herdr, Some(p)),
        (None, Some(p)) if codex_thread.is_none() => (Host::Tmux, Some(p)),
        _ => (Host::None, None),
    };
    let explicit = plan.or_else(|| std::env::var("AP_PLAN").ok().filter(|s| !s.is_empty()));
    let (key, terminal) = match (&explicit, &codex_thread, host, &source_pane) {
        (Some(name), ..) => (name.clone(), None),
        (None, Some(thread), _, _) if codex_pane.is_some() => (format!("codex-{thread}"), None),
        (None, _, Host::Herdr, Some(p)) => (format!("herdr-{p}"), herdr::terminal_of(p)),
        (None, _, Host::Tmux, Some(p)) => (format!("tmux-{p}"), None),
        (None, Some(thread), ..) => (format!("codex-{thread}"), None),
        _ => ("default".into(), None),
    };
    if source_pane.is_none() && (herdr::inside() || cline_daemon) {
        eprintln!(
            "ap: 이 명령이 실행된 Herdr pane을 확인할 수 없어 진행 창을 자동으로 열지 않습니다. Codex라면 스레드 이름이 정해진 뒤(첫 응답 후) 다시 시도됩니다. 지금 계획은 다른 pane에서 `ap --plan {key} view`로 볼 수 있습니다"
        );
    }
    let panes = match (host, &source_pane) {
        (Host::Herdr, Some(p)) => Panes::for_source("herdr", p),
        (Host::Tmux, Some(p)) => Panes::for_source("tmux", p),
        _ => None,
    };
    Ok(Ctx {
        store: Store::new(&root, &key),
        key,
        root,
        host,
        source_pane,
        terminal,
        panes,
    })
}

impl Ctx {
    /// Mutate the plan. A pane ID now backed by another terminal starts a fresh plan.
    fn update<T>(&self, event: &str, f: impl FnOnce(&mut Plan) -> Result<T>) -> Result<(Plan, T)> {
        if let (Some(term), Some(existing)) = (&self.terminal, self.store.load()?)
            && existing.terminal_id.as_ref().is_some_and(|t| t != term)
        {
            // A new terminal is a new session: keep the old plan as an archive.
            self.store.archive()?;
            self.store
                .update(&self.key, "new (pane reused by another terminal)", |_| {
                    Ok(())
                })?;
        }
        let terminal = self.terminal.clone();
        self.store.update(&self.key, event, |p| {
            if p.terminal_id.is_none() {
                p.terminal_id = terminal;
            }
            f(p)
        })
    }

    /// Point this pane's viewer at the current plan, opening one under the caller pane
    /// unless a viewer is already running for the pane or the user dismissed it.
    fn ensure_view(&self, force: bool) -> Result<Option<String>> {
        let (Some(source), Some(panes)) = (&self.source_pane, &self.panes) else {
            if force {
                bail!("Herdr/tmux pane 밖입니다. 다른 터미널에서 `ap view`를 실행하세요");
            }
            return Ok(None);
        };
        let auto_open = std::env::var("AP_AUTO_OPEN").as_deref() != Ok("0");
        let mut prior = panes.load().unwrap_or_default();
        if self.terminal.is_some() && prior.terminal_id != self.terminal {
            prior = PaneState::default();
        }
        let alive = prior
            .viewer
            .as_ref()
            .is_some_and(|v| panes.beating(&v.instance));
        let decision = pane::decide(&prior, alive, force, auto_open);
        let instance = (decision == Decision::Launch).then(new_instance);
        let terminal = self.terminal.clone();
        let plan_path = self.store.path.clone();
        // Always point the pane at the plan just changed; a live viewer switches to it.
        // When launching, record the instance first: a viewer exits once the pane state
        // stops naming it.
        panes.update(|s| {
            if terminal.is_some() && s.terminal_id != terminal {
                *s = PaneState::default();
                s.terminal_id = terminal.clone();
            }
            s.plan = plan_path;
            if let Some(id) = &instance {
                s.viewer = Some(Viewer {
                    instance: id.clone(),
                    ..Viewer::default()
                });
                s.suppressed = false;
            }
        })?;
        let Some(instance) = instance else {
            return Ok(None);
        };
        let size: u8 = std::env::var("AP_PANE_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);
        let command = viewer_command(&panes.path, &instance)?;
        let launched = match self.host {
            Host::Herdr => match herdr::idle_pane_below(source) {
                Some(pane) => herdr::run_in(&pane, &command, &instance),
                None => herdr::open_below(source, &self.root, &command, &instance, size),
            },
            Host::Tmux => tmux::open_below(source, &self.root, &command, &instance, size),
            Host::None => unreachable!("panes exist only for a known host"),
        };
        let mine = |s: &PaneState| s.viewer.as_ref().is_some_and(|v| v.instance == instance);
        match launched {
            Ok(viewer) => {
                let pane = viewer.pane.clone();
                panes.update(|s| {
                    if mine(s) {
                        s.viewer = Some(viewer);
                    }
                })?;
                Ok(Some(pane))
            }
            Err(e) => {
                panes.update(|s| {
                    if mine(s) {
                        s.viewer = None;
                    }
                })?;
                Err(e)
            }
        }
    }
}

fn new_instance() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}-{:x}", std::process::id(), nanos)
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Run this exact binary; a bare `ap` could resolve to another installed version.
fn viewer_command(pane_state: &std::path::Path, instance: &str) -> Result<String> {
    let exe = std::env::current_exe()?;
    Ok(format!(
        "{} view --pane-state {} --instance {}",
        shell_quote(exe.to_str().context("binary path encoding")?),
        shell_quote(pane_state.to_str().context("state path encoding")?),
        shell_quote(instance)
    ))
}

fn print_plan(plan: &Plan) {
    println!("{}", view::summary(Some(plan)));
}

fn main() {
    // Behave like other CLIs when output is piped into `head`: exit quietly instead
    // of panicking on a closed pipe.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    if let Err(e) = run() {
        eprintln!("ap: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    // Viewers launched on an explicit file or pane state need no pane resolution.
    if let Some(Cmd::View {
        file,
        once,
        instance,
        pane_state,
    }) = &cli.command
    {
        if let (Some(state), Some(id)) = (pane_state, instance) {
            return view::follow(&Panes::at(state.clone()), id);
        }
        if let Some(file) = file {
            let store = Store::at(file.clone());
            if *once {
                println!("{}", view::summary(store.load()?.as_ref()));
                return Ok(());
            }
            return view::watch(&store);
        }
    }
    if let Some(Cmd::Skill { action }) = &cli.command {
        let report = match action {
            SkillAction::Install { dry_run } => skill::install(*dry_run)?,
            SkillAction::Remove { dry_run } => skill::remove(*dry_run)?,
        };
        report.iter().for_each(|line| println!("{line}"));
        return Ok(());
    }
    let ctx = context(cli.plan)?;
    let command = cli.command.unwrap_or(Cmd::Status { json: false });
    let plan = match command {
        Cmd::Goal { text } => {
            ctx.update(&format!("goal {text}"), |p| {
                p.goal = Some(text.trim().to_string()).filter(|s| !s.is_empty());
                Ok(())
            })?
            .0
        }
        Cmd::Add { titles } => {
            ctx.update(&format!("add {}", titles.join(" | ")), |p| {
                titles.iter().try_for_each(|t| p.add(t).map(drop))
            })?
            .0
        }
        Cmd::Start { item } => {
            ctx.update(&format!("start {item}"), |p| {
                p.set(&item, State::Doing, None, None)
            })?
            .0
        }
        Cmd::Done { items } => {
            ctx.update(&format!("done {}", items.join(" ")), |p| {
                items
                    .iter()
                    .try_for_each(|i| p.set(i, State::Done, None, None).map(drop))
            })?
            .0
        }
        Cmd::Block {
            item,
            reason,
            needs,
        } => {
            ctx.update(&format!("block {item}: {reason}"), |p| {
                p.set(&item, State::Blocked, Some(reason.clone()), needs.clone())
            })?
            .0
        }
        Cmd::Cancel { item, reason } => {
            ctx.update(&format!("cancel {item}"), |p| {
                p.set(&item, State::Cancelled, reason.clone(), None)
            })?
            .0
        }
        Cmd::Todo { item } => {
            ctx.update(&format!("todo {item}"), |p| {
                p.set(&item, State::Todo, None, None)
            })?
            .0
        }
        Cmd::Rm { item } => {
            ctx.update(&format!("rm {item}"), |p| p.remove(&item).map(drop))?
                .0
        }
        Cmd::New => {
            // The pane's viewer keeps watching the same path and shows the fresh plan.
            ctx.store.archive()?;
            ctx.update("new", |_| Ok(()))?.0
        }
        Cmd::Status { json } => {
            let plan = ctx.store.load()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                println!("{}", view::summary(plan.as_ref()));
            }
            return Ok(());
        }
        Cmd::History => {
            match std::fs::read_to_string(ctx.store.history_path()) {
                Ok(s) => print!("{s}"),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => println!("이력 없음"),
                Err(e) => return Err(e.into()),
            }
            return Ok(());
        }
        Cmd::View { once, .. } => {
            if once {
                println!("{}", view::summary(ctx.store.load()?.as_ref()));
                return Ok(());
            }
            return view::watch(&ctx.store);
        }
        Cmd::Open => {
            match ctx.ensure_view(true)? {
                Some(pane) => println!("진행 창: {pane}"),
                None => println!("진행 창이 이미 열려 있습니다"),
            }
            return Ok(());
        }
        Cmd::Skill { .. } => unreachable!("handled before pane resolution"),
        Cmd::Close => {
            let Some(panes) = &ctx.panes else {
                println!("열린 진행 창이 없습니다");
                return Ok(());
            };
            let running = panes
                .load()
                .and_then(|s| s.viewer)
                .filter(|v| panes.beating(&v.instance));
            // The viewer exits by itself once the pane state no longer names it.
            panes.update(|s| {
                s.viewer = None;
                s.suppressed = true;
            })?;
            let message = match running {
                None => "열린 진행 창이 없습니다",
                Some(v) => {
                    let stopped = (0..30).any(|_| {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        !panes.beating(&v.instance)
                    });
                    if stopped {
                        "진행 창을 닫았습니다"
                    } else {
                        "진행 창에 종료를 요청했습니다"
                    }
                }
            };
            println!("{message}");
            return Ok(());
        }
    };
    print_plan(&plan);
    if !cli.no_view {
        // Display failure never undoes the recorded change.
        match ctx.ensure_view(false) {
            Ok(Some(pane)) => println!("진행 창: {pane}"),
            Ok(None) => {}
            Err(e) => eprintln!("ap: 기록은 저장됨, 진행 창 열기 실패: {e:#}"),
        }
    }
    Ok(())
}
