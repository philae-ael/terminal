use std::{
    io::{Read, Write},
    os::fd::AsRawFd,
};

use mio::{unix::SourceFd, Token};

use anyhow::{Context, Result};
use pty::{Pty, WinSizeExt};
use signal_hook::consts as sigconsts;
use signal_hook_mio::v0_8::Signals;
use stdfiles::{StdinRaw, StdoutRaw};

pub mod pty;
pub mod stdfiles;

fn main() -> Result<()> {
    let mut stdin = StdinRaw::new()?;
    let mut stdout = StdoutRaw::new()?;

    let mut pty = Pty::new(stdout.as_raw_fd().get_term_size()?)?;
    let mut poll = mio::Poll::new()?;

    poll.registry().register(
        &mut pty,
        mio::Token(0),
        mio::Interest::READABLE | mio::Interest::WRITABLE,
    )?;

    poll.registry().register(
        &mut SourceFd(&stdin.fd()),
        mio::Token(1),
        mio::Interest::READABLE,
    )?;

    let mut signals = Signals::new([sigconsts::SIGUSR1, sigconsts::SIGWINCH])
        .expect("Can't listen for signals in current thread");
    poll.registry()
        .register(&mut signals, Token(2), mio::Interest::READABLE)?;

    let mut events = mio::Events::with_capacity(1024);

    let mut buf = [0; 256];
    'outer: loop {
        match poll.poll(&mut events, None) {
            Ok(_) => (),
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => (),
            err => err.expect("Error while polling"),
        }

        for event in &events {
            match event.token() {
                Token(0) => {
                    // read from pty, rerender (write to stdout)
                    match pty.read(&mut buf) {
                        Ok(size) => {
                            let data = &buf[0..size];
                            stdout.write_all(data)?;
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => continue,
                        Err(err) => Err(err)?,
                    }
                }
                Token(1) => {
                    // read from stdin, write to pty
                    let size = stdin.read(&mut buf).context("Reading from stdin")?;
                    let data = &buf[0..size];
                    pty.write_all(data)?;
                }
                Token(2) => {
                    for signal in signals.pending() {
                        match signal {
                            sigconsts::SIGUSR1 => break 'outer,
                            sigconsts::SIGWINCH => {
                                pty.set_term_size(&stdout.as_raw_fd().get_term_size()?)?;
                            }
                            _ => (),
                        }
                    }
                }
                _ => (),
            }
        }
    }

    Ok(())
}
