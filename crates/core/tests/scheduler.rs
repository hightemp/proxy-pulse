use proxy_pulse_core::{
    model::{CheckSettings, Status},
    parser::ImportOptions,
    session::{self, Session},
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

struct Server {
    port: u16,
    stop: Arc<AtomicBool>,
    peak: Arc<AtomicUsize>,
    active: Arc<AtomicUsize>,
    starts: Arc<Mutex<Vec<Instant>>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Server {
    fn new(delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let peak = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let starts = Arc::new(Mutex::new(Vec::new()));
        let (thread_stop, thread_peak, thread_active, thread_starts) = (
            Arc::clone(&stop),
            Arc::clone(&peak),
            Arc::clone(&active),
            Arc::clone(&starts),
        );
        let handle = thread::spawn(move || {
            let mut handlers = Vec::new();
            while !thread_stop.load(Ordering::SeqCst) {
                if let Ok((mut stream, _)) = listener.accept() {
                    let (stop, peak, active, starts) = (
                        Arc::clone(&thread_stop),
                        Arc::clone(&thread_peak),
                        Arc::clone(&thread_active),
                        Arc::clone(&thread_starts),
                    );
                    handlers.push(thread::spawn(move || {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(count, Ordering::SeqCst);
                        starts.lock().unwrap().push(Instant::now());
                        let mut request = [0; 4096];
                        let read = stream.read(&mut request).unwrap_or(0);
                        assert!(request[..read].starts_with(b"GET http://check.invalid/"));
                        let deadline = Instant::now() + delay;
                        while Instant::now() < deadline && !stop.load(Ordering::SeqCst) {
                            thread::sleep(Duration::from_millis(5));
                        }
                        let body = b"{\"ip\":\"198.51.100.7\"}";
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.write_all(body);
                        active.fetch_sub(1, Ordering::SeqCst);
                    }));
                } else {
                    thread::sleep(Duration::from_millis(5));
                }
            }
            for handler in handlers {
                handler.join().unwrap();
            }
        });
        Self {
            port,
            stop,
            peak,
            active,
            starts,
            thread: Some(handle),
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            handle.join().unwrap();
        }
    }
}

fn prepared(server: &Server, count: usize) -> session::SharedSession {
    let mut state = Session::default();
    state
        .preview(
            &vec![format!("http://127.0.0.1:{}", server.port); count].join("\n"),
            &ImportOptions::default(),
        )
        .unwrap();
    state.commit_import(false, true, false).unwrap();
    Arc::new(Mutex::new(state))
}

fn wait_until(timeout: Duration, predicate: impl Fn() -> bool) {
    let deadline = Instant::now() + timeout;
    while !predicate() {
        assert!(Instant::now() < deadline, "Condition timed out");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn scheduler_obeys_concurrency_and_global_request_rate() {
    let server = Server::new(Duration::from_millis(240));
    let state = prepared(&server, 6);
    let ids = state.lock().unwrap().entries.iter().map(|e| e.id).collect();
    let settings = CheckSettings {
        url: "http://check.invalid/".into(),
        concurrency: 2,
        rate_limit: 10,
        ..CheckSettings::default()
    };
    session::start(Arc::clone(&state), ids, settings, false).unwrap();
    wait_until(Duration::from_secs(5), || !state.lock().unwrap().running);
    assert!(state
        .lock()
        .unwrap()
        .entries
        .iter()
        .all(|e| e.status == Status::Working));
    assert!(server.peak.load(Ordering::SeqCst) <= 2);
    let starts = server.starts.lock().unwrap();
    assert_eq!(starts.len(), 6);
    assert!(starts
        .windows(2)
        .all(|pair| pair[1].duration_since(pair[0]) >= Duration::from_millis(90)));
}

#[test]
fn cancellation_releases_workers_and_prevents_mutation_during_a_run() {
    let server = Server::new(Duration::from_secs(8));
    let state = prepared(&server, 8);
    let ids = state.lock().unwrap().entries.iter().map(|e| e.id).collect();
    let settings = CheckSettings {
        url: "http://check.invalid/".into(),
        concurrency: 2,
        rate_limit: 100,
        ..CheckSettings::default()
    };
    session::start(Arc::clone(&state), ids, settings, false).unwrap();
    wait_until(Duration::from_secs(2), || {
        server.active.load(Ordering::SeqCst) == 2
    });
    assert!(state.lock().unwrap().clear(&[]).is_err());
    let start = Instant::now();
    state.lock().unwrap().control.as_ref().unwrap().cancel();
    wait_until(Duration::from_secs(1), || !state.lock().unwrap().running);
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(state
        .lock()
        .unwrap()
        .entries
        .iter()
        .all(|e| e.status == Status::Cancelled));
    state.lock().unwrap().clear(&[]).unwrap();
    assert!(state.lock().unwrap().entries.is_empty());
}
