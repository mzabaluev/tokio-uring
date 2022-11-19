use std::env;

use tokio_uring::net::UnixStream;

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() <= 1 {
        panic!("no addr specified");
    }

    let socket_addr: &String = &args[1];

    tokio_uring::start(async {
        let stream = UnixStream::connect(socket_addr).await.unwrap();
        let buf = vec![1u8; 128];

        let result = stream.write(buf).await;
        let (written, buf) = result.unwrap();
        println!("written: {}", written);

        let result = stream.read(buf).await;
        let (read, buf) = result.unwrap();
        println!("read: {:?}", &buf[..read]);
    });
}
