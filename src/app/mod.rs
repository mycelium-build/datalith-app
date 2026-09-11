pub mod actions;
pub mod assets;
pub mod docs;
pub mod fonts;
pub mod keymap;
pub mod menus;
pub mod preferences;
pub mod settings;
mod state;
pub mod system;
pub mod update;
pub mod version;

use std::path::PathBuf;

pub use state::AppState;

pub fn init(cx: &mut gpui_kit::App) {
    gpui_kit::init(cx);
    cx.set_http_client(std::sync::Arc::new(reqwest_client::ReqwestClient::new()));
    update::init(cx);
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_default()
        .join(crate::channel::Channel::current().stem())
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    use futures::AsyncReadExt;

    #[gpui_kit::test]
    #[allow(
        clippy::needless_pass_by_ref_mut,
        reason = "GPUI test macro requires a mutable test context"
    )]
    fn initialized_app_downloads_remote_images(cx: &mut gpui_kit::TestAppContext) {
        const IMAGE: &[u8] = include_bytes!("../../assets/icons/base.svg");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/cover.svg", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let started = Instant::now();
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if started.elapsed() >= Duration::from_secs(5) {
                            return false;
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("image server: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(line, "GET /cover.svg HTTP/1.1\r\n");
            loop {
                line.clear();
                assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                if line == "\r\n" {
                    break;
                }
            }
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", IMAGE.len()).unwrap();
            stream.write_all(IMAGE).unwrap();
            true
        });

        cx.update(super::init);
        let client = cx.update(|cx| cx.http_client());
        let result = pollster::block_on(async {
            let mut response = client.get(&url, ().into(), true).await?;
            anyhow::ensure!(
                response.status().is_success(),
                "image request: {}",
                response.status()
            );
            let mut bytes = Vec::new();
            response.body_mut().read_to_end(&mut bytes).await?;
            anyhow::Ok(bytes)
        });
        let requested = server.join().unwrap();
        assert_eq!(result.unwrap(), IMAGE);
        assert!(requested, "application must request the remote image");
    }
}
