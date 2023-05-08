use std::{
    fs::File,
    io::{Error, ErrorKind, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::CommandExt,
    },
    process::{Child, Command, Stdio},
};

use libc::winsize;
use mio::{event::Source, unix::SourceFd};
use nix::{
    fcntl::{fcntl, FcntlArg, OFlag},
    pty::OpenptyResult,
    sys::{
        signal::Signal,
        termios::{self, InputFlags},
    },
    unistd::Pid,
};
use signal_hook::consts as sigconsts;
use signal_hook_mio::v0_8::Signals;

pub struct Pty {
    pub child: Child,
    pub file: File,
    pub signals: Signals,
}

use terminal_common::WinSizeExt;

// Heavily based on https://github.com/alacritty/alacritty/blob/master/alacritty_terminal/src/tty/unix.rs

impl Pty {
    pub fn new(size: winsize) -> std::io::Result<Pty> {
        let OpenptyResult { master, slave } = nix::pty::openpty(Some(&size), None)?;

        if let Ok(mut termios) = termios::tcgetattr(master) {
            termios.input_flags.set(InputFlags::IUTF8, true);
            let _ = termios::tcsetattr(master, termios::SetArg::TCSANOW, &termios);
        }

        let mut command = Command::new("/usr/bin/sh");

        command.stdin(unsafe { Stdio::from_raw_fd(slave) });
        command.stdout(unsafe { Stdio::from_raw_fd(slave) });
        command.stderr(unsafe { Stdio::from_raw_fd(slave) });

        unsafe {
            // There is a fork call in pre_exec
            command.pre_exec(move || {
                let err = libc::setsid();
                if err == -1 {
                    return Err(Error::new(ErrorKind::Other, "Failed to set session id"));
                }

                libc::close(slave);
                libc::close(master);

                libc::signal(libc::SIGCHLD, libc::SIG_DFL);
                libc::signal(libc::SIGHUP, libc::SIG_DFL);
                libc::signal(libc::SIGINT, libc::SIG_DFL);
                libc::signal(libc::SIGQUIT, libc::SIG_DFL);
                libc::signal(libc::SIGTERM, libc::SIG_DFL);
                libc::signal(libc::SIGALRM, libc::SIG_DFL);

                Ok(())
            });
        }

        // setup signals
        let signals = Signals::new([sigconsts::SIGCHLD]).expect("error preparing signal handling");

        match command.spawn() {
            Ok(child) => {
                let fd_flags = OFlag::from_bits_truncate(fcntl(master, FcntlArg::F_GETFL)?);
                fcntl(master, FcntlArg::F_SETFL(fd_flags | OFlag::O_NONBLOCK))?;

                Ok(Self {
                    file: unsafe { File::from_raw_fd(master) },
                    child,
                    signals,
                })
            }
            Err(err) => Err(Error::new(
                err.kind(),
                format!(
                    "Failed to spawn command '{}': {}",
                    command.get_program().to_string_lossy(),
                    err
                ),
            )),
        }
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        let _ = nix::sys::signal::kill(Pid::from_raw(self.child.id() as _), Signal::SIGHUP);
        let _ = self.child.wait();
    }
}

impl Read for Pty {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buf)
    }
}

impl Write for Pty {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Source for Pty {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        registry.register(&mut SourceFd(&self.file.as_raw_fd()), token, interests)?;
        registry.register(&mut self.signals, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        registry.reregister(&mut SourceFd(&self.file.as_raw_fd()), token, interests)?;
        registry.reregister(&mut self.signals, token, interests)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> std::io::Result<()> {
        registry.deregister(&mut SourceFd(&self.file.as_raw_fd()))?;
        registry.deregister(&mut self.signals)
    }
}

impl WinSizeExt for Pty {
    fn get_term_size(&self) -> std::io::Result<winsize> {
        self.file.as_raw_fd().get_term_size()
    }

    fn set_term_size(&self, win: &winsize) -> std::io::Result<()> {
        self.file.as_raw_fd().set_term_size(win)
    }
}
