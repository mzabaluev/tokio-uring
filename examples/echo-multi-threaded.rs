use tokio_uring::net::TcpListener;
use tokio_uring::runtime::Runtime;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::spawn_local;
use std::sync::Arc;

fn run(thread: &str, listener: Arc<TcpListener>) {
    let mut rt = Runtime::new().unwrap();

    rt.block_on(async {
        loop {
            let mut socket = listener.accept().await.unwrap();

            println!("ACCEPTED {}", thread);

            spawn_local(async move {
                let mut buf = vec![0_u8; 4096];

                // Read data and write it back
                loop {

                    let n = socket
                        .read(&mut buf)
                        .await
                        .unwrap();

                    if n == 0 {
                        println!("connection closed gracefully");
                        return;
                    }

                    println!("received {} bytes", n);

                    socket
                        .write_all(&buf[..n])
                        .await
                        .unwrap();

                    socket.flush().await.unwrap();
                }
            });
        }
    });
}

fn main() {
    let addr = "127.0.0.1:8080".parse().unwrap();
    let listener1 = Arc::new(TcpListener::bind(addr).unwrap());
    let listener2 = listener1.clone();

    let th = std::thread::spawn(move || {
        run("thread 1", listener2);
    });

    run("thread 0", listener1);

    th.join().unwrap();
}