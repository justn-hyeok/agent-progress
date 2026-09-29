//! Observe one native frontend's RPC connection while retaining the shared server.
//! Only start/resume/fork response identity is retained; conversation bytes are forwarded.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    collections::HashMap,
    io::{BufReader, Read, Write},
    os::unix::{
        fs::OpenOptionsExt,
        net::{UnixListener, UnixStream},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Id {
    Number(i64),
    Text(String),
}
impl Id {
    fn key(&self) -> String {
        match self {
            Self::Number(n) => format!("number:{n}"),
            Self::Text(s) => format!("text:{s}"),
        }
    }
}
#[derive(Debug, Deserialize)]
struct Message {
    id: Option<Id>,
    method: Option<String>,
    result: Option<Response>,
    params: Option<Params>,
}
#[derive(Debug, Deserialize)]
struct Params {
    #[serde(rename = "threadId")]
    thread_id: Option<uuid::Uuid>,
    ephemeral: Option<bool>,
}
#[derive(Debug, Deserialize)]
struct Response {
    thread: Option<Thread>,
}
#[derive(Debug, Clone, Deserialize)]
struct Thread {
    id: uuid::Uuid,
    path: Option<PathBuf>,
    cwd: Option<PathBuf>,
    #[serde(default)]
    ephemeral: bool,
}

struct Forward<R, W> {
    reader: R,
    writer: W,
}
impl<R: Read, W: Write> Read for Forward<R, W> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = self.reader.read(buffer)?;
        self.writer.write_all(&buffer[..count])?;
        self.writer.flush()?;
        Ok(count)
    }
}

// This only decodes a copy for metadata inspection. Raw HTTP/WebSocket bytes are
// forwarded untouched, including authentication, fragmentation and control frames.
struct WsRead<R> {
    raw: R,
    remaining: u64,
    position: u64,
    mask: [u8; 4],
    data: bool,
}
impl<R: Read> WsRead<R> {
    fn new(mut raw: R) -> std::io::Result<Self> {
        let mut last = [0u8; 4];
        for _ in 0..65536 {
            let mut byte = [0];
            raw.read_exact(&mut byte)?;
            last.rotate_left(1);
            last[3] = byte[0];
            if last == *b"\r\n\r\n" {
                return Ok(Self {
                    raw,
                    remaining: 0,
                    position: 0,
                    mask: [0; 4],
                    data: false,
                });
            }
        }
        Err(std::io::Error::other(
            "native upgrade header exceeded inspection limit",
        ))
    }
}
impl<R: Read> Read for WsRead<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        loop {
            if self.remaining == 0 {
                let mut header = [0; 2];
                if self.raw.read(&mut header[..1])? == 0 {
                    return Ok(0);
                }
                self.raw.read_exact(&mut header[1..])?;
                if header[0] & 0x70 != 0 {
                    return Err(std::io::Error::other(
                        "native frame extension not inspected",
                    ));
                }
                self.data = matches!(header[0] & 0xf, 0..=2);
                self.remaining = match header[1] & 0x7f {
                    126 => {
                        let mut size = [0; 2];
                        self.raw.read_exact(&mut size)?;
                        u16::from_be_bytes(size).into()
                    }
                    127 => {
                        let mut size = [0; 8];
                        self.raw.read_exact(&mut size)?;
                        u64::from_be_bytes(size)
                    }
                    size => size.into(),
                };
                self.mask = [0; 4];
                if header[1] & 0x80 != 0 {
                    self.raw.read_exact(&mut self.mask)?;
                }
                self.position = 0;
                if self.remaining == 0 {
                    continue;
                }
            }
            let mut control = [0; 125];
            let buffer = if self.data {
                &mut *out
            } else {
                &mut control[..]
            };
            let limit = self.remaining.min(buffer.len() as u64) as usize;
            let count = self.raw.read(&mut buffer[..limit])?;
            if count == 0 {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            for (index, byte) in buffer[..count].iter_mut().enumerate() {
                *byte ^= self.mask[((self.position + index as u64) % 4) as usize];
            }
            self.position += count as u64;
            self.remaining -= count as u64;
            if self.data {
                return Ok(count);
            }
        }
    }
}

#[derive(Default)]
struct Exchanges {
    requests: HashMap<String, String>,
    early: HashMap<String, Thread>,
    active: Option<Thread>,
    latest: Option<String>,
}
impl Exchanges {
    fn request(&mut self, message: Message) -> Option<Thread> {
        if message.method.as_deref() == Some("turn/start") {
            let id = message.params.and_then(|p| p.thread_id);
            return self.active.as_ref().filter(|t| Some(t.id) == id).cloned();
        }
        let (Some(id), Some(method)) = (message.id, message.method) else {
            return None;
        };
        let key = id.key();
        let selects = message
            .params
            .as_ref()
            .is_none_or(|p| p.ephemeral != Some(true))
            && matches!(
                method.as_str(),
                "thread/start" | "thread/resume" | "thread/fork"
            );
        if selects {
            self.latest = Some(key.clone());
        }
        if let Some(thread) = self.early.remove(&key) {
            if !selects {
                return None;
            }
            self.active = Some(thread.clone());
            return Some(thread);
        }
        if self.requests.len() < 128 {
            self.requests.insert(key, method);
        }
        None
    }
    fn response(&mut self, message: Message) -> Option<Thread> {
        if message.method.is_some() {
            return None;
        }
        let id = message.id?;
        let key = id.key();
        let method = self.requests.remove(&key);
        let thread = message.result.and_then(|r| r.thread)?;
        // Codex opens ephemeral helper threads (for example title generation) on
        // the same frontend connection. They are not the visible conversation.
        if thread.ephemeral {
            return None;
        }
        if let Some(method) = method {
            if !matches!(
                method.as_str(),
                "thread/start" | "thread/resume" | "thread/fork"
            ) || self.latest.as_ref() != Some(&key)
            {
                return None;
            }
            self.active = Some(thread.clone());
            return Some(thread);
        }
        if self.early.len() < 128 {
            self.early.insert(key, thread);
        }
        None
    }
}

/// Native override/embedded/remote modes retain their own transport. Do not create
/// a viewer that cannot receive an exact frontend session in those modes.
pub fn can_observe(args: &[String]) -> bool {
    if !crate::terminal::interactive_args("codex", args)
        || args.iter().any(|a| {
            matches!(
                a.as_str(),
                "--no-daemon"
                    | "--help"
                    | "-h"
                    | "--version"
                    | "-V"
                    | "-c"
                    | "--config"
                    | "--enable"
                    | "--disable"
                    | "--search"
                    | "--dangerously-bypass-hook-trust"
            ) || a.starts_with("--config=")
                || a.starts_with("--enable=")
                || a.starts_with("--disable=")
                || (a.starts_with("-c") && a.len() > 2)
        })
    {
        return false;
    }
    if let Some(index) = args
        .iter()
        .position(|a| a == "--remote" || a.starts_with("--remote="))
    {
        return args[index]
            .strip_prefix("--remote=")
            .or_else(|| args.get(index + 1).map(String::as_str))
            .is_some_and(|remote| remote.starts_with("unix://") && remote.len() > 7);
    }
    true
}

pub fn launch(args: Vec<String>) -> Result<()> {
    let mut args = args;
    let mut native = Command::new("codex");
    native.env("AP_AUTO_OPEN", "1");
    let portable = crate::terminal::slot();
    if (std::env::var("HERDR_ENV").as_deref() != Ok("1") && portable.is_none())
        || !can_observe(&args)
    {
        native.args(args);
        return Err(native.exec().into());
    }
    let remote_index = args
        .iter()
        .position(|a| a == "--remote" || a.starts_with("--remote="));
    let explicit_remote = remote_index.and_then(|i| {
        args[i]
            .strip_prefix("--remote=")
            .map(str::to_owned)
            .or_else(|| args.get(i + 1).cloned())
    });
    if explicit_remote
        .as_ref()
        .is_some_and(|remote| !remote.starts_with("unix://"))
    {
        native.args(args);
        return Err(native.exec().into());
    }
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")))
        .context("Codex home unavailable")?;
    let backend = explicit_remote
        .as_ref()
        .map(|r| PathBuf::from(r.trim_start_matches("unix://")))
        .unwrap_or_else(|| home.join("app-server-control/app-server-control.sock"));
    if explicit_remote.is_none() && !backend.exists() {
        let mut start = Command::new("codex");
        start.args(["app-server", "daemon", "start"]);
        crate::herdr::bounded(start, None).context("shared Codex server did not start")?;
    }
    if let Some(index) = remote_index {
        let inline = args[index].starts_with("--remote=");
        args.remove(index);
        if !inline {
            args.remove(index);
        }
    }
    let pane = if portable.is_some() {
        String::new()
    } else {
        std::env::var("HERDR_PANE_ID").context("source pane unavailable")?
    };
    let owner = std::process::id();
    if let Some(slot) = &portable {
        crate::terminal::validate_slot(slot)?;
        ensure!(
            std::fs::read_to_string(slot.join("owner"))? == owner.to_string(),
            "terminal owner mismatch"
        );
    } else {
        let source = crate::herdr::call(&["pane", "process-info", "--pane", &pane])?;
        ensure!(
            source["result"]["process_info"]["foreground_processes"]
                .as_array()
                .is_some_and(|ps| ps.iter().any(|p| p["pid"].as_u64() == Some(owner.into()))),
            "launcher is not running in the claimed source pane"
        );
    }
    let initial = std::env::current_dir()?.canonicalize()?;
    let mut root = initial.clone();
    for i in 0..args.len() {
        let selected = if matches!(args[i].as_str(), "--cd" | "-C") {
            args.get(i + 1).cloned()
        } else {
            args[i].strip_prefix("--cd=").map(str::to_owned)
        };
        if let Some(selected) = selected {
            let selected = PathBuf::from(selected);
            root = if selected.is_absolute() {
                selected
            } else {
                initial.join(selected)
            }
            .canonicalize()?;
            if matches!(args[i].as_str(), "--cd" | "-C") {
                args[i + 1] = root.display().to_string();
            } else {
                args[i] = format!("--cd={}", root.display());
            }
        }
    }
    let directory = tempfile::Builder::new()
        .prefix("ap-client-")
        .tempdir_in("/tmp")?;
    let socket = directory.path().join("server.sock");
    let diagnostic_dir = root.join(".agent-progress/bridges");
    ensure!(
        !root.join(".agent-progress").is_symlink() && !diagnostic_dir.is_symlink(),
        "refusing symlink progress storage"
    );
    std::fs::create_dir_all(&diagnostic_dir)?;
    let diagnostic_path =
        diagnostic_dir.join(format!("client-{owner}-{}.log", uuid::Uuid::new_v4()));
    let diagnostic = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(diagnostic_path)?;
    let mut proxy = Command::new(std::env::current_exe()?)
        .args([
            "codex-client",
            "--pane",
            &pane,
            "--owner",
            &owner.to_string(),
        ])
        .arg("--socket")
        .arg(&socket)
        .arg("--root")
        .arg(&root)
        .arg("--backend")
        .arg(&backend)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(diagnostic))
        .process_group(0)
        .current_dir(&root)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !directory.path().join("ready").exists() {
        ensure!(
            proxy.try_wait()?.is_none(),
            "Codex connection observer failed to start"
        );
        if Instant::now() >= deadline {
            let _ = proxy.kill();
            let _ = proxy.wait();
            anyhow::bail!("Codex connection observer did not become ready");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let kept = directory.keep();
    native
        .arg("--remote")
        .arg(format!("unix://{}", socket.display()));
    if !args
        .iter()
        .any(|a| a == "--cd" || a == "-C" || a.starts_with("--cd="))
    {
        native.arg("--cd").arg(&root);
    }
    native.args(args);
    native.current_dir(&root);
    let error = native.exec();
    let _ = proxy.kill();
    let _ = proxy.wait();
    for file in ["server.sock", "ready"] {
        let _ = std::fs::remove_file(kept.join(file));
    }
    let _ = std::fs::remove_dir(kept);
    Err(error.into())
}

fn bind(
    thread: Thread,
    pane: &str,
    owner: u32,
    root: &Path,
    backend_home: Option<&Path>,
    marker: &Path,
) {
    let result = (|| -> Result<()> {
        let cwd = thread.cwd.context("native thread cwd unavailable")?;
        let cwd = cwd.canonicalize()?;
        let mut path = thread.path;
        let deadline = Instant::now() + Duration::from_secs(2);
        while path
            .as_ref()
            .is_none_or(|p| crate::live::session_header(p).is_err())
            && Instant::now() < deadline
        {
            if let Some(home) = backend_home
                && let Ok(found) = crate::live::find_session(home, thread.id)
            {
                path = Some(found);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let path = path.context("native transcript creation pending")?;
        let selected: serde_json::Value = serde_json::from_slice(&crate::recovery::read(marker)?)?;
        if selected["session"] != thread.id.to_string() {
            return Ok(());
        }
        if let Some(slot) = crate::terminal::slot() {
            crate::terminal::publish(
                &slot,
                crate::terminal::Selection {
                    session: thread.id,
                    rollout: path,
                    cwd,
                    owner,
                },
            )?;
            return Ok(());
        }
        crate::bridge::register_client(pane, owner, thread.id, &path, &cwd, root, marker)
            .context("native client registration failed")?;
        if crate::settings::load_for_cwd(&cwd)?.auto_open
            && std::env::var("AP_AUTO_OPEN").as_deref() != Ok("0")
        {
            crate::herdr::open_quiet(Some(pane.into()), true).context("observer opening failed")?;
        }
        Ok(())
    })();
    // Observation never blocks or changes a native turn.
    if let Err(error) = result {
        eprintln!("progress connection unavailable: {error:#}");
    }
}

pub fn serve(
    socket: &Path,
    backend_socket: &Path,
    pane: &str,
    owner: u32,
    root: &Path,
) -> Result<()> {
    let directory = socket
        .parent()
        .context("missing observer socket directory")?;
    ensure!(
        directory
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("ap-client-"))
            && !directory.is_symlink()
            && socket.file_name().is_some_and(|n| n == "server.sock"),
        "invalid observer socket path"
    );
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            for name in ["server.sock", "ready", "selection.json"] {
                let _ = std::fs::remove_file(self.0.join(name));
            }
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.into());
    let listener = UnixListener::bind(socket)?;
    listener.set_nonblocking(true)?;
    std::fs::write(directory.join("ready"), b"ready")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let frontend = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(e) => return Err(e.into()),
        }
    };
    // macOS accepts can inherit the listener's nonblocking flag. The forwarding
    // readers must wait for later native frames rather than treating WouldBlock as EOF.
    frontend.set_nonblocking(false)?;
    let closing = frontend.try_clone()?;
    let backend = UnixStream::connect(backend_socket)?;
    let backend_closing = backend.try_clone()?;
    let state = Arc::new(Mutex::new(Exchanges::default()));
    let marker = directory.join("selection.json");
    let (observations, received) = std::sync::mpsc::sync_channel::<Thread>(8);
    let observed_pane = pane.to_owned();
    let observed_root = root.to_owned();
    let observed_marker = marker.clone();
    let backend_home = backend_socket
        .parent()
        .filter(|p| p.file_name().is_some_and(|n| n == "app-server-control"))
        .and_then(Path::parent)
        .map(Path::to_owned);
    let observer = std::thread::spawn(move || {
        for thread in received {
            bind(
                thread,
                &observed_pane,
                owner,
                &observed_root,
                backend_home.as_deref(),
                &observed_marker,
            );
        }
    });
    let requests = state.clone();
    let request_observations = observations.clone();
    let request_marker = marker.clone();
    let forward = Forward {
        reader: frontend.try_clone()?,
        writer: backend.try_clone()?,
    };
    let outgoing = std::thread::spawn(move || {
        let raw = BufReader::new(forward);
        let Ok(mut inspected) = WsRead::new(raw) else {
            return;
        };
        let messages = serde_json::Deserializer::from_reader(&mut inspected).into_iter::<Message>();
        for message in messages {
            let Ok(message) = message else {
                break;
            };
            let selected = requests.lock().unwrap().request(message);
            if let Some(thread) = selected {
                let _ = crate::recovery::write(
                    &request_marker,
                    serde_json::json!({"session":thread.id,"owner":owner})
                        .to_string()
                        .as_bytes(),
                    true,
                );
                let _ = request_observations.try_send(thread);
            }
        }
        // A future native message shape may be unrecognized. Keep forwarding it.
        let mut raw = inspected.raw.into_inner();
        let _ = std::io::copy(&mut raw.reader, &mut raw.writer);
        let _ = raw.writer.shutdown(std::net::Shutdown::Both);
    });
    let forward = Forward {
        reader: backend,
        writer: frontend,
    };
    let mut inspected = WsRead::new(BufReader::new(forward))?;
    let mut messages = serde_json::Deserializer::from_reader(&mut inspected).into_iter::<Message>();
    for message in messages.by_ref() {
        let Ok(message) = message else {
            break;
        };
        let selected = state.lock().unwrap().response(message);
        if let Some(thread) = selected {
            let _ = crate::recovery::write(
                &marker,
                serde_json::json!({"session":thread.id,"owner":owner})
                    .to_string()
                    .as_bytes(),
                true,
            );
            let _ = observations.try_send(thread);
        }
    }
    drop(messages);
    let mut raw = inspected.raw.into_inner();
    let _ = std::io::copy(&mut raw.reader, &mut raw.writer);
    let _ = closing.shutdown(std::net::Shutdown::Both);
    let _ = backend_closing.shutdown(std::net::Shutdown::Both);
    // Closing the frontend ends its read side and wakes the request forwarder.
    let _ = outgoing.join();
    drop(observations);
    let _ = observer.join();
    drop(listener);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(opcode: u8, fin: bool, payload: &[u8], masked: bool) -> Vec<u8> {
        let mut bytes = vec![opcode | if fin { 0x80 } else { 0 }];
        let flag = if masked { 0x80 } else { 0 };
        if payload.len() < 126 {
            bytes.push(flag | payload.len() as u8);
        } else if payload.len() <= u16::MAX as usize {
            bytes.push(flag | 126);
            bytes.extend((payload.len() as u16).to_be_bytes());
        } else {
            bytes.push(flag | 127);
            bytes.extend((payload.len() as u64).to_be_bytes());
        }
        let mask = [7, 4, 2, 9];
        if masked {
            bytes.extend(mask);
        }
        bytes.extend(
            payload
                .iter()
                .enumerate()
                .map(|(i, b)| if masked { b ^ mask[i % 4] } else { *b }),
        );
        bytes
    }

    #[test]
    fn inspects_fragmented_masked_messages_without_changing_transport_bytes() {
        let body = br#"{"id":7,"method":"turn/start","params":{"threadId":"6382a2cc-c36d-4533-af02-111111111111"}}"#;
        let mut wire = b"GET / HTTP/1.1\r\nUpgrade: websocket\r\n\r\n".to_vec();
        wire.extend(frame(1, false, &body[..20], true));
        wire.extend(frame(9, true, b"not JSON", true));
        wire.extend(frame(0, true, &body[20..], true));
        let forward = Forward {
            reader: std::io::Cursor::new(wire.clone()),
            writer: Vec::new(),
        };
        let mut observed = WsRead::new(BufReader::with_capacity(7, forward)).unwrap();
        let messages = serde_json::Deserializer::from_reader(&mut observed)
            .into_iter::<Message>()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].method.as_deref(), Some("turn/start"));
        assert_eq!(observed.raw.into_inner().writer, wire);
    }

    #[test]
    fn large_native_history_is_skipped_and_forwarded_without_adopting_foreign_threads() {
        let id = uuid::Uuid::new_v4();
        let body = format!(
            r#"{{"id":4,"result":{{"thread":{{"id":"{id}","path":"/rollout","cwd":"/project","turns":[{{"text":"{}"}}]}}}}}}"#,
            "x".repeat(128 * 1024)
        );
        let mut wire = b"HTTP/1.1 101 Switching Protocols\r\n\r\n".to_vec();
        wire.extend(frame(1, true, body.as_bytes(), false));
        let forward = Forward {
            reader: std::io::Cursor::new(wire.clone()),
            writer: Vec::new(),
        };
        let mut observed = WsRead::new(BufReader::new(forward)).unwrap();
        let response = serde_json::Deserializer::from_reader(&mut observed)
            .into_iter::<Message>()
            .next()
            .unwrap()
            .unwrap();
        let mut state = Exchanges::default();
        assert!(state.response(response).is_none());
        assert!(state.active.is_none());
        let mut tail = Vec::new();
        observed.read_to_end(&mut tail).unwrap();
        assert_eq!(observed.raw.into_inner().writer, wire);
    }

    #[test]
    fn late_replies_and_reading_other_threads_do_not_change_the_selected_session() {
        let a = uuid::Uuid::new_v4();
        let b = uuid::Uuid::new_v4();
        let reply = |key, id| {
            serde_json::from_value::<Message>(serde_json::json!({"id":key,"result":{"thread":{"id":id,"path":"/rollout","cwd":"/project"}}})).unwrap()
        };
        let mut state = Exchanges::default();
        state.request(serde_json::from_str(r#"{"id":1,"method":"thread/start"}"#).unwrap());
        state.request(serde_json::from_str(r#"{"id":2,"method":"thread/resume"}"#).unwrap());
        assert!(state.response(reply(1, a)).is_none());
        assert_eq!(state.response(reply(2, b)).unwrap().id, b);
        state.request(serde_json::from_str(r#"{"id":3,"method":"thread/read"}"#).unwrap());
        assert!(state.response(reply(3, a)).is_none());
        assert_eq!(state.active.as_ref().unwrap().id, b);
        let turn = |id| {
            serde_json::from_value::<Message>(
                serde_json::json!({"id":4,"method":"turn/start","params":{"threadId":id}}),
            )
            .unwrap()
        };
        assert!(state.request(turn(a)).is_none());
        assert_eq!(state.request(turn(b)).unwrap().id, b);
    }

    #[test]
    fn native_ephemeral_title_helpers_do_not_replace_the_visible_conversation() {
        let main = uuid::Uuid::new_v4();
        let helper = uuid::Uuid::new_v4();
        let mut state = Exchanges::default();
        state.request(serde_json::from_str(r#"{"id":1,"method":"thread/start"}"#).unwrap());
        let reply = serde_json::json!({"id":1,"result":{"thread":{"id":main,"path":"/rollout","cwd":"/project","ephemeral":false}}});
        assert_eq!(
            state
                .response(serde_json::from_value(reply).unwrap())
                .unwrap()
                .id,
            main
        );
        state.request(
            serde_json::from_str(r#"{"id":2,"method":"thread/start","params":{"ephemeral":true}}"#)
                .unwrap(),
        );
        let reply = serde_json::json!({"id":2,"result":{"thread":{"id":helper,"path":null,"cwd":"/project","ephemeral":true}}});
        assert!(
            state
                .response(serde_json::from_value(reply).unwrap())
                .is_none()
        );
        assert_eq!(state.active.unwrap().id, main);
    }

    #[test]
    fn selects_only_matching_client_start_resume_fork_responses() {
        let id = uuid::Uuid::new_v4();
        let response = format!(
            r#"{{"id":1,"result":{{"thread":{{"id":"{id}","path":"/session","cwd":"/project"}}}}}}"#
        );
        let mut state = Exchanges::default();
        assert!(
            state
                .response(serde_json::from_str(&response).unwrap())
                .is_none()
        );
        assert!(
            state
                .request(serde_json::from_str(r#"{"id":2,"method":"thread/read"}"#).unwrap())
                .is_none()
        );
        assert_eq!(
            state
                .request(serde_json::from_str(r#"{"id":1,"method":"thread/start"}"#).unwrap())
                .unwrap()
                .id,
            id
        );
        assert!(
            state
                .response(serde_json::from_str(&response).unwrap())
                .is_none()
        );
        assert!(
            state
                .request(serde_json::from_str(r#"{"id":1,"method":"thread/read"}"#).unwrap())
                .is_none()
        );
    }
}
