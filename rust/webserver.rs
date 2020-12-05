use std::borrow::Cow;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;

//run: cargo test webserver -- --nocapture

#[test]
fn run() {
    start_server(8080, "./public").unwrap();
}

pub fn start_server(port: u16, public_root: &str) -> io::Result<()> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    eprintln!("Starting server on 'http://{}'", address);
    let listener = TcpListener::bind(address)?;

    for stream_result in listener.incoming() {
        match stream_result.map(|stream| handle_client(public_root, stream)) {
            Ok(Err(e)) => eprintln!("Error handling client: {}", e),
            Err(e) => eprintln!("Connection failed: {}", e),
            _ => {}
        }
    }
    Ok(())
}

// Example request
//
// |GET / HTTP/1.1
// |Host: localhost:8080
// |User-Agent: curl/7.73.0
// |Accept: */*

//fn handle_client(mut stream: TcpStream) -> io::Result<()> {
//    // linux/limits.h max path is 4096 + 'GET ' + 'HTTP'
//    let mut buffer = [0u8; 4104];
//    stream.read(&mut buffer)?;
//    io::stdout().write(&buffer);
//    Ok(())
//}

fn handle_client(public: &str, mut stream: TcpStream) -> io::Result<()> {
    // linux/limits.h max path is 4096 + 'GET ' + 'HTTP'
    let mut buffer = [0; 4014]; // Big enough to hold first line

    //stream.read_exact(&mut buffer)?;
    // I am not sure what to do about clippy recommending to use 'read_exact'
    // I do not need the entire buffer read and 'read_exact' blocks
    #[allow(clippy::unused_io_aount)]
    stream.read(&mut buffer)?;

    let as_str = match std::str::from_utf8(&buffer) {
        Ok(s) => s,
        Err(e) => {
            let (good, _bad) = buffer.split_at(e.valid_up_to());
            // Do not check UT8 validity twice
            unsafe { std::str::from_utf8_unchecked(good) }
        }
    };

    let request = as_str
        .find(" HTTP")
        .map(|index| &as_str["GET ".len()..index])
        .unwrap_or("Unsupported request or buffer not large enough");
    let response = serve_static_file(public, request);
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

macro_rules! define_str {
    ($joined:ident = {
            $head:ident = $head_val:literal +
            $body:ident = $body_val:literal
        }) => {
        const $joined: &str = concat!($head_val, $body_val);
        const $head: &str = $head_val;
        const $body: &str = $body_val;
    };
}
define_str! {
    NOT_FOUND = {
        NOT_FOUND_HEADER = "HTTP/1.1 404 Not Found\r\n\r\n"
            +
        NOT_FOUND_BODY   = "<h1>404 File Not Found.</h1>"
    }
}
//const FORBIDDEN: &str = concat!(
//    //
//    "HTTP/1.1 403 FORBIDDEN\r\n\r\n",
//    "<h1>403 Forbidden.</h1>",
//);
const OK_HEADER: &str = "HTTP/1.1 200 OK\r\n\r\n";

fn serve_static_file(public_root: &str, request: &str) -> String {
    let paths = [
        ([public_root, request, "/index.html"].join(""), OK_HEADER),
        ([public_root, request].join(""), OK_HEADER),
        ([public_root, "/404.html"].join(""), NOT_FOUND_HEADER),
    ];

    let take_first = paths
        .iter()
        .map(|(loc, h)| (Path::new(loc), h))
        .filter(|(path, _)| path.is_file())
        .map(|(path, header)| {
            fs::read_to_string(path)
                .map(|contents| [header, contents.as_str()].join(""))
                .map(Cow::Owned)
                .unwrap_or(Cow::Borrowed(NOT_FOUND))
        })
        .next();

    take_first.unwrap_or(Cow::Borrowed(NOT_FOUND)).into_owned()
}
