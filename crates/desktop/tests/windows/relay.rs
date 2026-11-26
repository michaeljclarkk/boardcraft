//! A disposable child-only TCP route. A cut drops new connections before opening
//! the upstream socket. The peer always connects directly to the real host.
use std::{
    io,
    net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
        mpsc,
    },
    thread,
    time::Duration,
};

pub struct Relay {
    pub port: u16,
    pub cut: Arc<AtomicBool>,
    pub dropped: Arc<AtomicUsize>,
    stop: mpsc::Sender<()>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Relay {
    pub fn start(host: u16) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let (stop, rx) = mpsc::channel();
        let cut = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicUsize::new(0));
        let (offline, rejected) = (cut.clone(), dropped.clone());
        let worker = thread::spawn(move || {
            let mut streams = Vec::new();
            while rx.try_recv().is_err() {
                match listener.accept() {
                    Ok((_child, _)) if offline.load(SeqCst) => {
                        rejected.fetch_add(1, SeqCst);
                    }
                    Ok((child, _)) => streams.push(thread::spawn(move || forward(child, host))),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(_) => panic!("child relay accept failed"),
                }
            }
            for stream in streams {
                stream.join().unwrap();
            }
        });
        Self {
            port,
            cut,
            dropped,
            stop,
            worker: Some(worker),
        }
    }
}

fn forward(mut child: TcpStream, host: u16) {
    let timeout = Duration::from_secs(8);
    // On Windows accepted sockets can inherit nonblocking mode from the listener.
    child.set_nonblocking(false).unwrap();
    let mut upstream =
        TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, host)), timeout)
            .unwrap();
    for s in [&child, &upstream] {
        s.set_read_timeout(Some(timeout)).unwrap();
        s.set_write_timeout(Some(timeout)).unwrap();
    }
    let mut request = child.try_clone().unwrap();
    let mut dest = upstream.try_clone().unwrap();
    thread::scope(|scope| {
        scope.spawn(move || {
            let _ = io::copy(&mut request, &mut dest);
            let _ = dest.shutdown(Shutdown::Write);
        });
        let _ = io::copy(&mut upstream, &mut child);
        let _ = child.shutdown(Shutdown::Both);
    });
}

impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        self.worker.take().unwrap().join().unwrap();
    }
}
