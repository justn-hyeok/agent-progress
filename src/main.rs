mod herdr;
mod store;
mod tmux;
mod view;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
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
    },
    /// 현재 pane 아래에 진행 창을 엽니다 (자동 열기 억제 해제)
    Open,
    /// 이 계획의 진행 창을 닫고 자동 열기를 멈춥니다
    Close,
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
    let tmux_pane = std::env::var("TMUX_PANE").ok().filter(|p| !p.is_empty());
    let (host, source_pane) = match (herdr_pane.or(codex_pane.clone()), tmux_pane) {
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
    if source_pane.is_none() && herdr::inside() {
        eprintln!(
            "ap: 이 명령이 실행된 Herdr pane을 확인할 수 없어 진행 창을 자동으로 열지 않습니다. Codex라면 스레드 이름이 정해진 뒤(첫 응답 후) 다시 시도됩니다. 지금 계획은 다른 pane에서 `ap --plan {key} view`로 볼 수 있습니다"
        );
    }
    Ok(Ctx {
        store: Store::new(&root, &key),
        key,
        root,
        host,
        source_pane,
        terminal,
    })
}

impl Ctx {
    /// Mutate the plan. A pane ID now backed by another terminal starts a fresh plan.
    fn update<T>(&self, event: &str, f: impl FnOnce(&mut Plan) -> Result<T>) -> Result<(Plan, T)> {
        if let (Some(term), Some(existing)) = (&self.terminal, self.store.load()?)
            && existing.terminal_id.as_ref().is_some_and(|t| t != term)
        {
            // The viewer watches the same path, so it carries over to the fresh plan.
            self.store.archive()?;
            self.store
                .update(&self.key, "new (pane reused by another terminal)", |p| {
                    p.viewer = existing.viewer.clone();
                    p.view_suppressed = existing.view_suppressed;
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

    fn viewer_alive(&self, v: &Viewer) -> bool {
        match self.host {
            Host::Herdr => herdr::alive(v),
            Host::Tmux => tmux::alive(v),
            Host::None => false,
        }
    }

    /// Open the viewer under the caller pane unless one is already running or the user dismissed it.
    fn ensure_view(&self, plan: &Plan, force: bool) -> Result<Option<String>> {
        let Some(source) = &self.source_pane else {
            if force {
                bail!("Herdr/tmux pane 밖입니다. 다른 터미널에서 `ap view`를 실행하세요");
            }
            return Ok(None);
        };
        if !force && (plan.view_suppressed || std::env::var("AP_AUTO_OPEN").as_deref() == Ok("0")) {
            return Ok(None);
        }
        if let Some(viewer) = &plan.viewer {
            // q and `ap close` clear the record, so a recorded but dead viewer was closed
            // from outside (e.g. Herdr's own close). Respect that like q.
            if self.viewer_alive(viewer) || !force {
                return Ok(None);
            }
        }
        let size: u8 = std::env::var("AP_PANE_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);
        let viewer = match self.host {
            Host::Herdr => match herdr::idle_pane_below(source) {
                Some(pane) => herdr::run_in(&pane, &self.store.path)?,
                None => herdr::open_below(source, &self.root, &self.store.path, size)?,
            },
            Host::Tmux => tmux::open_below(source, &self.root, &self.store.path, size)?,
            Host::None => return Ok(None),
        };
        let pane = viewer.pane.clone();
        self.store.update(&self.key, "", |p| {
            p.viewer = Some(viewer);
            p.view_suppressed = false;
            Ok(())
        })?;
        Ok(Some(pane))
    }
}

fn print_plan(plan: &Plan) {
    println!("{}", view::summary(Some(plan)));
}

fn main() {
    if let Err(e) = run() {
        eprintln!("ap: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
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
            let old = ctx.store.load()?;
            if let Some(v) = old.as_ref().and_then(|p| p.viewer.clone()) {
                // Keep the viewer pane; it watches the same path and will show the new plan.
                ctx.store.archive()?;
                let (p, _) = ctx.update("new", |p| {
                    p.viewer = Some(v);
                    Ok(())
                })?;
                p
            } else {
                ctx.store.archive()?;
                ctx.update("new", |_| Ok(()))?.0
            }
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
        Cmd::View { file, once } => {
            let store = file.map(Store::at).unwrap_or(ctx.store);
            if once {
                println!("{}", view::summary(store.load()?.as_ref()));
                return Ok(());
            }
            return view::watch(&store);
        }
        Cmd::Open => {
            let (plan, _) = ctx.update("", |p| {
                p.view_suppressed = false;
                Ok(())
            })?;
            match ctx.ensure_view(&plan, true)? {
                Some(pane) => println!("진행 창: {pane}"),
                None => println!("진행 창이 이미 열려 있습니다"),
            }
            return Ok(());
        }
        Cmd::Close => {
            let plan = ctx.store.load()?.context("이 계획이 없습니다")?;
            let closed = match (&plan.viewer, ctx.host) {
                (Some(v), Host::Herdr) => herdr::close(v)?,
                (Some(v), Host::Tmux) => tmux::close(v)?,
                _ => false,
            };
            ctx.update("", |p| {
                p.viewer = None;
                p.view_suppressed = true;
                Ok(())
            })?;
            println!(
                "{}",
                if closed {
                    "진행 창을 닫았습니다"
                } else {
                    "열린 진행 창이 없습니다"
                }
            );
            return Ok(());
        }
    };
    print_plan(&plan);
    if !cli.no_view {
        // Display failure never undoes the recorded change.
        match ctx.ensure_view(&plan, false) {
            Ok(Some(pane)) => println!("진행 창: {pane}"),
            Ok(None) => {}
            Err(e) => eprintln!("ap: 기록은 저장됨, 진행 창 열기 실패: {e:#}"),
        }
    }
    Ok(())
}
