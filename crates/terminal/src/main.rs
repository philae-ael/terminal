use mio::Token;

use signal_hook::consts as sigconsts;
use signal_hook_mio::v0_8::Signals;
use terminal_echo::Echo;
use terminal_tty::pty::Pty;

use terminal_common::{Term, WinSizeExt};

#[derive(thiserror::Error, Debug)]
pub enum ProcessEventError {
    #[error("The underlying process died, exiting")]
    ProcessDied,
    #[error("An unknown io error arised: {0}")]
    IoError(#[from] std::io::Error),
    #[error("A signal required the event loop to exit")]
    SigBreak,
}

pub fn process_event(
    event: &mio::event::Event,
    pty: &mut Pty,
    echo: &mut Echo,
    signals: &mut Signals,
) -> Result<(), ProcessEventError> {
    use ProcessEventError::*;
    match event.token() {
        Token(0) => match echo.gather_outputs(pty) {
            Ok(_) => Ok(()),
            Err(err) if err.raw_os_error() == Some(5) => Err(ProcessDied),
            Err(err) => Err(IoError(err)),
        },
        Token(1) => match echo.forward_inputs(pty) {
            Ok(_) => Ok(()),
            Err(err) if err.raw_os_error() == Some(5) => Err(ProcessDied),
            Err(err) => Err(IoError(err)),
        },
        Token(2) => {
            for signal in signals.pending() {
                match signal {
                    sigconsts::SIGUSR1 => Err(SigBreak)?,
                    sigconsts::SIGWINCH => {
                        pty.set_term_size(&echo.get_term_size()?)?;
                    }
                    _ => (),
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn main() -> Result<(), ProcessEventError> {
    let mut echo = Echo::new()?;
    let mut pty = Pty::new(echo.get_term_size()?)?;
    let mut poll = mio::Poll::new()?;
    let mut signals = Signals::new([sigconsts::SIGUSR1, sigconsts::SIGWINCH])
        .expect("Can't listen for signals in current thread");

    poll.registry()
        .register(&mut pty, mio::Token(0), mio::Interest::READABLE)?;
    poll.registry()
        .register(&mut echo, mio::Token(1), mio::Interest::READABLE)?;
    poll.registry()
        .register(&mut signals, Token(2), mio::Interest::READABLE)?;

    let mut events = mio::Events::with_capacity(1024);

    loop {
        match poll.poll(&mut events, None) {
            Ok(_) => (),
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => (),
            err => err.expect("Error while polling"),
        }

        for event in &events {
            process_event(event, &mut pty, &mut echo, &mut signals)?;
        }
    }
}
