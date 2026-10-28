//! A tiny HTTP/1.1 mock server for tests: serves queued responses in order and
//! records every request it receives.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).expect("request body is JSON")
    }
}

#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Reply {
    pub fn json(status: u16, body: serde_json::Value) -> Self {
        Reply {
            status,
            headers: vec![("content-type".into(), "application/json".into())],
            body: body.to_string(),
        }
    }

    pub fn header(mut self, k: &str, v: &str) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
}

pub struct MockServer {
    pub url: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

impl MockServer {
    pub async fn start(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let queue = Arc::new(Mutex::new(VecDeque::from(replies)));
        let recorded = requests.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let queue = queue.clone();
                let recorded = recorded.clone();
                tokio::spawn(async move {
                    let Some(req) = read_request(&mut sock).await else {
                        return;
                    };
                    recorded.lock().unwrap().push(req);
                    let reply = queue.lock().unwrap().pop_front().unwrap_or(Reply {
                        status: 599,
                        headers: vec![],
                        body: "no reply queued".into(),
                    });
                    let mut out = format!(
                        "HTTP/1.1 {} X\r\ncontent-length: {}\r\nconnection: close\r\n",
                        reply.status,
                        reply.body.len()
                    );
                    for (k, v) in &reply.headers {
                        out.push_str(&format!("{k}: {v}\r\n"));
                    }
                    out.push_str("\r\n");
                    out.push_str(&reply.body);
                    let _ = sock.write_all(out.as_bytes()).await;
                    let _ = sock.shutdown().await;
                });
            }
        });
        MockServer { url, requests }
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }
}

async fn read_request(sock: &mut tokio::net::TcpStream) -> Option<Recorded> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = sock.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let path = first.next()?.to_string();
    let headers: Vec<(String, String)> = lines
        .filter(|l| !l.is_empty())
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
        .collect();
    let len: usize = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    while buf.len() < header_end + len {
        let n = sock.read(&mut chunk).await.ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buf[header_end..]).to_string();
    Some(Recorded {
        method,
        path,
        headers,
        body,
    })
}
