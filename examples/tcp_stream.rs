use std::{env, net::SocketAddr};

use tokio_uring::net::TcpStream;

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() <= 1 {
        panic!("no addr specified");
    }

    let socket_addr: SocketAddr = args[1].parse().unwrap();

    tokio_uring::start(async {
        let stream = TcpStream::connect(socket_addr).await.unwrap();
        let buf = vec![1u8; 128];

        let result = stream.write(buf).await;
        let (written, buf) = result.unwrap();
        println!("written: {}", written);

        let result = stream.read(buf).await;
        let (read, buf) = result.unwrap();
        println!("read: {:?}", &buf[..read]);
    });
}
