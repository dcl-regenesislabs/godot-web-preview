//! `tokio::fs` on native. On web, the same API over Emscripten's in-memory FS:
//! tokio::fs needs a blocking thread pool, and MEMFS calls return immediately anyway.

#[cfg(not(target_family = "wasm"))]
pub use tokio::fs::*;

#[cfg(target_family = "wasm")]
pub use wasm::*;

#[cfg(target_family = "wasm")]
mod wasm {
    use std::io::{self, Read, Seek, Write};
    use std::path::Path;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};

    pub use std::fs::Metadata;

    pub async fn metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
        std::fs::metadata(path)
    }
    pub async fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }
    pub async fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
        std::fs::read_to_string(path)
    }
    pub async fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
        std::fs::write(path, contents)
    }
    pub async fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }
    pub async fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }
    pub async fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::remove_file(path)
    }
    pub async fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    pub struct File(std::fs::File);

    impl File {
        pub async fn open(path: impl AsRef<Path>) -> io::Result<File> {
            std::fs::File::open(path).map(File)
        }
        pub async fn create(path: impl AsRef<Path>) -> io::Result<File> {
            std::fs::File::create(path).map(File)
        }
        pub async fn metadata(&self) -> io::Result<Metadata> {
            self.0.metadata()
        }
        pub async fn sync_all(&self) -> io::Result<()> {
            Ok(())
        }
    }

    impl AsyncRead for File {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            let n = self.get_mut().0.read(buf.initialize_unfilled())?;
            buf.advance(n);
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for File {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(self.get_mut().0.write(buf))
        }
        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(self.get_mut().0.flush())
        }
        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncSeek for File {
        fn start_seek(self: Pin<&mut Self>, position: io::SeekFrom) -> io::Result<()> {
            self.get_mut().0.seek(position).map(|_| ())
        }
        fn poll_complete(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
            Poll::Ready(self.get_mut().0.stream_position())
        }
    }
}
