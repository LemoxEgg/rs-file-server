use std::{
    collections::{HashMap, VecDeque},
    fmt::Display,
    fs::File,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use flate2::Compression;
use urlencoding;

const VERSION: &str = "HTTP/1.1";
const ENCODINGS: [&str; 1] = ["gzip"];
const HTML_HEAD: &str = include_str!("head.html");
const HTML_FOOT: &str = include_str!("foot.html");

#[derive(Debug, Default)]
struct Response {
    version: String,
    code: u32,
    message: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug, Default)]
struct Request<'a> {
    _version: &'a [u8],
    method: &'a [u8],
    target: &'a [u8],
    headers: HashMap<&'a [u8], &'a [u8]>,
    _body: &'a [u8],
}

#[cfg(not(windows))]
#[link(name = "ip")]
extern "C" {
    fn ip(interface_name: *const i8, ip_array: *mut u8) -> u32;
}

fn main() {
    #[cfg(not(windows))]
    unsafe {
        let interface_name = "wlp4s0\0"; // C string with null terminator
        let mut ip_array = [0; 4];
        let success = ip(interface_name.as_ptr() as *const i8, ip_array.as_mut_ptr());
        if success == 0 {
            println!(
                "hosting server on: {}.{}.{}.{}:6565",
                ip_array[0], ip_array[1], ip_array[2], ip_array[3]
            );
        } else {
            println!("ip address not found");
        }
    }

    #[cfg(windows)]
    {
        let ip = local_ip_address::local_ip().unwrap();
        println!("hosting server on: {}:6565", ip);
    }

    //init some variables
    let listener = TcpListener::bind("0.0.0.0:6565").unwrap();
    listener.set_nonblocking(false).unwrap();
    let mut thread_handles: VecDeque<(JoinHandle<io::Result<()>>, Instant)> = VecDeque::new();

    //check args for -f flag, if found, check whether the next arg,
    //if it's a directory, set it as the current dir and return None,
    //otherwise return it as a single file
    let single_file_mode = if let Some(n) = std::env::args().position(|s| s == "-f") {
        if let Some(s) = std::env::args().nth(n + 1) {
            if Path::new(&s).is_file() {
                println!("hosting single file: {}", s);
                Some(s.into())
            } else if Path::new(&s).is_dir() {
                std::env::set_current_dir(std::env::args().nth(n + 1).unwrap()).unwrap();
                println!("hosting directory: {}", s);
                None
            } else {
                panic!("{} is neither a file nor a directory", s);
            }
        } else {
            None
        }
    } else {
        None
    };
    //check args for -c flag, if found, compress any file set over the network
    let compression = std::env::args().any(|s| s == "-c");
    if compression {
        println!("compression enabled");
    }

    //main loop
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let single_file_mode_clone = single_file_mode.clone();
                thread_handles.push_back((
                    thread::spawn(move || {
                        handle_connection(stream, single_file_mode_clone, compression)
                    }),
                    Instant::now(),
                ));
            }
            Err(e) => {
                if e.kind() != io::ErrorKind::WouldBlock {
                    println!("error: {}", e);
                }
            }
        }
        while !thread_handles.is_empty() && thread_handles[0].1.elapsed() > Duration::from_secs(3) {
            let (handle, _) = thread_handles.pop_front().unwrap();
            handle.join().unwrap().unwrap();
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    single_file: Option<PathBuf>,
    compression: bool,
) -> io::Result<()> {
    println!(
        "new connection from: {}:80",
        stream
            .peer_addr()
            .map_or_else(|_| String::from("unknown!"), |a| a.to_string())
    );

    let mut read_buff = [0u8; 1000]; //pretty small all things considered, but this is only an exercise
    let request = match stream.read(&mut read_buff) {
        Ok(n) => {
            parse_request(&read_buff[..n]).unwrap() //TEMP
        }
        Err(e) => crash(Box::new(e), "response failed: ", stream),
    };

    let response = match request.method {
        b"GET" => {
            println!(
                "{}: GET {}",
                stream.peer_addr().unwrap(),
                String::from_utf8_lossy(&urlencoding::decode_binary(request.target))
            );
            check_target(request, single_file, compression)
        }
        //b"POST" => {
        //    create_file(request)
        //}
        _ => Response {
            version: VERSION.to_string(),
            code: 405,
            message: "Method Not Allowed".to_string(),
            ..Default::default()
        },
    };

    let resp_line = format!(
        "{} {} {}\r\n",
        response.version, response.code, response.message
    );

    //format every header to a single string and append a \r\n to the end of all the headers
    let resp_headers = response
        .headers
        .iter()
        .fold(String::new(), |s, (h, v)| s + h + ": " + v + "\r\n")
        + "\r\n";

    stream.write_all(resp_line.as_bytes())?;
    stream.write_all(resp_headers.as_bytes())?;
    stream.write_all(&response.body)?;

    println!("response sent: {} {}", response.code, response.message);
    Ok(())
}

/// parses and returns an request
fn parse_request(bytes: &[u8]) -> Result<Request, String> {
    //get the indexes of each \r\n in start/end pairs
    let indexes = bytes
        .iter()
        .enumerate()
        .fold(vec![0], |mut state, (index, b)| {
            if *b == b'\r' {
                state.push(index);
            } else if *b == b'\n' {
                state.push(index + 1);
            }
            state
        });

    //collect slices of the original body from the indexes
    let mut body: &[u8] = &[];
    let mut req = Vec::new();
    let mut iter = indexes.iter();
    while let Some(start) = iter.next() {
        if let Some(end) = iter.next() {
            req.push(&bytes[*start..*end]);
        } else {
            body = &bytes[*start..];
            break;
        }
    }

    let mut req_line = req[0].split(|b| *b == b' ');
    let headers = req
        .iter()
        .skip(1)
        .take_while(|h| !h.is_empty())
        .map(|s| {
            let header = s.split(|b| *b == b':').next().unwrap_or_default();
            let value = s
                .split(|b| *b == b':')
                .nth(1)
                .unwrap_or(b" ".as_ref())
                .strip_prefix(b" ")
                .unwrap();
            (header, value)
        })
        .collect();

    Ok(Request {
        method: req_line.next().unwrap_or_default(),
        target: req_line.next().unwrap_or_default(),
        _version: req_line.next().unwrap_or_default(),
        headers,
        _body: body,
    })
}

/// checks if the requested file exists, if so return an apropriate response
///
/// if the file does not exists returns a 404 error
/// url encoding standard just uses the ascii code in hex after a %
fn check_target(request: Request, single_file: Option<PathBuf>, compression: bool) -> Response {
    if let Some(single_file) = single_file {
        let mut headers = HashMap::new();
        let body = read_n_compress_file(&single_file, &request, compression, &mut headers);

        headers.insert(
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        );
        headers.insert("Content-Length".to_string(), body.len().to_string());
        headers.insert(
            "Content-Disposition".to_string(),
            format!(
                "attachment; filename=\"{}\"",
                single_file.file_name().unwrap().to_string_lossy()
            ),
        );

        return Response {
            version: VERSION.to_string(),
            code: 200,
            message: "OK".to_string(),
            headers,
            body,
        };
    }

    let working_dir = std::env::current_dir().unwrap();
    let mut filename =
        String::from_utf8_lossy(&urlencoding::decode_binary(request.target)).into_owned();
    filename.remove(0);
    if filename.is_empty() {
        filename.push('.');
    }
    let path = working_dir.join(&filename);
    //println!("path \"{}\"", path.display());
    //println!("filename \"{}\"", &filename);

    if path.is_file() {
        let mut headers = HashMap::new();
        let body = read_n_compress_file(&path, &request, compression, &mut headers);

        //check the file type
        let extention = path.extension().unwrap_or_default();
        let content_type = match extention.to_str().unwrap() {
            "html" => "text/html",
            "css" => "text/css",
            "js" => "text/javascript",
            "wasm" => "application/wasm",
            _ => {
                "application/octet-stream" //this is good to download files but not for anything else
            }
        };

        headers.insert("Content-Type".to_string(), content_type.to_string());
        headers.insert("Content-Length".to_string(), body.len().to_string());

        return Response {
            version: VERSION.to_string(),
            code: 200,
            message: "OK".to_string(),
            headers,
            body,
        };
    } else if path.is_dir() {
        //current_dir.push(path);
        let mut body = Vec::new();
        body.extend_from_slice(HTML_HEAD.as_bytes());

        body.extend_from_slice(link("..", "..").as_bytes());
        for f in path.read_dir().unwrap() {
            let os_name = f.unwrap().file_name();
            let name = os_name.to_str().unwrap();
            body.extend_from_slice(link(&format!("./{}/{}", &filename, &name), name).as_bytes());
        }

        body.extend_from_slice(HTML_FOOT.as_bytes());

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/html".to_string());
        headers.insert("Content-Length".to_string(), body.len().to_string());

        return Response {
            version: VERSION.to_string(),
            code: 200,
            message: "OK".to_string(),
            headers,
            body,
        };
    }

    Response {
        version: VERSION.to_string(),
        code: 404,
        message: "Not Found".to_string(),
        ..Default::default()
    }
}

// read the given file, if the compression flag
// is set compress it if the client supports it
// and add the Content-Encoding header
fn read_n_compress_file(
    file_path: &PathBuf,
    request: &Request,
    compression: bool,
    headers: &mut HashMap<String, String>,
) -> Vec<u8> {
    let mut data = Vec::new();
    File::open(file_path)
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();

    if let Some(encodings) = request.headers.get(b"Accept-Encoding".as_ref()) {
        if compression
            && String::from_utf8_lossy(encodings)
                .split(", ")
                .any(|ce| ENCODINGS[0] == ce)
        {
            let mut buff: Vec<u8> = Vec::with_capacity(1000);
            let mut encoder = flate2::GzBuilder::new()
                .operating_system(3)
                .buf_read(data.as_slice(), Compression::default());

            if let Ok(amount) = encoder.read_to_end(&mut buff) {
                data = buff[..amount].to_vec();
                headers.insert("Content-Encoding".to_string(), "gzip".to_string());
            }
        }
    }
    data
}

fn crash(error: Box<dyn Display>, message: &str, stream: TcpStream) -> ! {
    stream.shutdown(std::net::Shutdown::Both).unwrap();
    panic!("{}{}", message, error);
}

fn link(path: &str, text: &str) -> String {
    format!("<a href=\"{}\">{}</a><br>", urlencoding::encode(path), text)
}
