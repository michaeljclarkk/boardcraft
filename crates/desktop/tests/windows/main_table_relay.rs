//! One-request loopback connections; fault flags affect only this fixture's table route.
use std::{
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering::SeqCst},
        mpsc,
    },
    thread,
    time::Duration,
};

pub struct Relay {
    pub port: u16,
    pub fault: Arc<AtomicUsize>, // 1: next table GET, 2: next POST acknowledgment
    pub saves: Arc<AtomicUsize>,
    stop: mpsc::Sender<()>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Relay {
    pub fn start(host: u16) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let fault = Arc::new(AtomicUsize::new(0));
        let saves = Arc::new(AtomicUsize::new(0));
        let (stop, rx) = mpsc::channel();
        let (f, s) = (fault.clone(), saves.clone());
        let worker = thread::spawn(move || {
            while rx.try_recv().is_err() {
                match listener.accept() {
                    Ok((stream, _)) => forward(stream, host, &f, &s),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => panic!("table relay accept failed"),
                }
            }
        });
        Self {
            port,
            fault,
            saves,
            stop,
            worker: Some(worker),
        }
    }
}

fn forward(mut child: TcpStream, host: u16, fault: &AtomicUsize, saves: &AtomicUsize) {
    child.set_nonblocking(false).unwrap();
    child
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    child
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut buf = [0; 4096];
    let head_end = loop {
        let n = child.read(&mut buf).unwrap();
        if n == 0 {
            return;
        }
        bytes.extend_from_slice(&buf[..n]);
        if let Some(ix) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
            break ix + 4;
        }
        assert!(bytes.len() < 65536);
    };
    let head = std::str::from_utf8(&bytes[..head_end]).unwrap().to_owned();
    let first = head.lines().next().unwrap();
    let read = first.starts_with("GET ") && first.contains("/main-table ");
    let save = first.starts_with("POST ") && first.contains("/main-table-fold ");
    let length = head
        .lines()
        .find_map(|l| {
            l.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                .map(|(_, v)| v.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    assert!(length <= 1024 * 1024);
    while bytes.len() < head_end + length {
        let n = child.read(&mut buf).unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&buf[..n]);
    }
    let refused =
        || b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    if read && fault.compare_exchange(1, 0, SeqCst, SeqCst).is_ok() {
        child.write_all(refused()).unwrap();
        return;
    }
    if save {
        saves.fetch_add(1, SeqCst);
    }
    let mut upstream = TcpStream::connect((Ipv4Addr::LOCALHOST, host)).unwrap();
    upstream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    for line in head
        .lines()
        .filter(|l| !l.is_empty() && !l.to_ascii_lowercase().starts_with("connection:"))
    {
        upstream.write_all(line.as_bytes()).unwrap();
        upstream.write_all(b"\r\n").unwrap();
    }
    upstream.write_all(b"Connection: close\r\n\r\n").unwrap();
    upstream
        .write_all(&bytes[head_end..head_end + length])
        .unwrap();
    let mut response = Vec::new();
    upstream.read_to_end(&mut response).unwrap();
    if save && fault.compare_exchange(2, 0, SeqCst, SeqCst).is_ok() {
        child.write_all(refused()).unwrap();
    } else {
        child.write_all(&response).unwrap();
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        self.worker.take().unwrap().join().unwrap();
    }
}
