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

