use std::{
    fs::File,
    io::{Read, StdinLock, StdoutLock, Write},
    os::fd::{AsRawFd, FromRawFd, RawFd},
};

use mio::{event::Source, unix::SourceFd};
use nix::{
    fcntl::{fcntl, FcntlArg, OFlag},
    sys::termios,
};

use terminal_common::{Term, WinSizeExt};

pub struct Echo<'a> {
    stdin: StdinRaw<'a>,
    stdout: StdoutRaw<'a>,
}

impl<'a> Echo<'a> {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            stdin: StdinRaw::new()?,
            stdout: StdoutRaw::new()?,
        })
    }
}

impl<'a> Read for Echo<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stdin.read(buf)
    }
}

impl<'a> Write for Echo<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stdout.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stdout.flush()
    }
}

impl<'a> Term for Echo<'a> {}

impl<'a> WinSizeExt for Echo<'a> {
    fn get_term_size(&self) -> std::io::Result<libc::winsize> {
        self.stdout.as_raw_fd().get_term_size()
    }

    fn set_term_size(&self, win: &libc::winsize) -> std::io::Result<()> {
        self.stdout.as_raw_fd().set_term_size(win)
    }
}

impl<'a> Source for Echo<'a> {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        registry.register(&mut SourceFd(&self.stdin.fd()), token, interests)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        registry.reregister(&mut SourceFd(&self.stdin.fd()), token, interests)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> std::io::Result<()> {
        registry.deregister(&mut SourceFd(&self.stdin.fd()))
    }
}

/// An unbuffered, raw, reader from stdout
struct StdinRaw<'a> {
    stdin: StdinLock<'a>,
    file: File,
    termios: termios::Termios,
    fcntl_flags: OFlag,
}

/// An unbuffered, raw, reader from stdout
struct StdoutRaw<'a> {
    stdout: StdoutLock<'a>,
    file: File,
}

impl<'a> StdinRaw<'a> {
    fn new() -> Result<Self, std::io::Error> {
        let stdin = std::io::stdin().lock();
        let fd = stdin.as_raw_fd();

        let termios = termios::tcgetattr(fd)?;
        let fcntl_flags = OFlag::from_bits_truncate(fcntl(fd, FcntlArg::F_GETFL)?);

        let mut termios_new = termios.clone();

        termios::cfmakeraw(&mut termios_new);
        termios::tcsetattr(fd, termios::SetArg::TCSANOW, &termios_new)?;
        fcntl(fd, FcntlArg::F_SETFL(fcntl_flags | OFlag::O_NONBLOCK))?;

        let file = unsafe { File::from_raw_fd(fd) };

        Ok(Self {
            termios,
            fcntl_flags,
            file,
            stdin,
        })
    }
    fn fd(&self) -> RawFd {
        self.stdin.as_raw_fd()
    }
}
impl<'a> Drop for StdinRaw<'a> {
    fn drop(&mut self) {
        let fd = self.fd();
        let _ = termios::tcsetattr(fd, termios::SetArg::TCSANOW, &self.termios);
        let _ = fcntl(fd, FcntlArg::F_SETFL(self.fcntl_flags));
    }
}

impl<'a> StdoutRaw<'a> {
    fn new() -> Result<Self, std::io::Error> {
        let stdout = std::io::stdout().lock();
        let fd = stdout.as_raw_fd();

        let file = unsafe { File::from_raw_fd(fd) };

        Ok(Self { file, stdout })
    }
}

impl<'a> AsRawFd for StdoutRaw<'a> {
    fn as_raw_fd(&self) -> RawFd {
        self.stdout.as_raw_fd()
    }
}

impl<'a> Write for StdoutRaw<'a> {
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }

    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.write(buf)
    }
}

impl<'a> Read for StdinRaw<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buf)
    }
}
