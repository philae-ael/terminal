use std::{
    fs::File,
    io::{Read, StdinLock, StdoutLock, Write},
    os::fd::{AsRawFd, FromRawFd, RawFd},
};

use nix::{
    fcntl::{fcntl, FcntlArg, OFlag},
    sys::termios,
};

use crate::pty::WinSizeExt;

/// An unbuffered, raw, reader from stdout
pub struct StdinRaw<'a> {
    stdin: StdinLock<'a>,
    file: File,
    termios: termios::Termios,
    fcntl_flags: OFlag,
}

/// An unbuffered, raw, reader from stdout
pub struct StdoutRaw<'a> {
    stdout: StdoutLock<'a>,
    file: File,
}

impl<'a> StdinRaw<'a> {
    pub fn new() -> Result<Self, std::io::Error> {
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
    pub fn fd(&self) -> RawFd {
        self.stdin.as_raw_fd()
    }
}

impl<'a> Read for StdinRaw<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buf)
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
    pub fn new() -> Result<Self, std::io::Error> {
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

impl<'a> WinSizeExt for StdoutRaw<'a> {
    fn get_term_size(&self) -> std::io::Result<libc::winsize> {
        self.as_raw_fd().get_term_size()
    }

    fn set_term_size(&self, win: &libc::winsize) -> std::io::Result<()> {
        self.as_raw_fd().set_term_size(win)
    }
}
