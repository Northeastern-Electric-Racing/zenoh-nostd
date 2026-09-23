use core::cell::RefCell;

use embassy_net::tcp::{TcpReader, TcpSocket, TcpWriter};
use embedded_io_async::{Read, Write};
use zenoh_nostd::platform::*;

use crate::BufferPoolDrop;

pub struct EmbassyTcpLink<'buf, 'net> {
    socket: TcpSocket<'buf, 'net>,
    mtu: u16,

    idx: usize,
    pool: &'buf RefCell<dyn BufferPoolDrop>,
}

impl<'buf, 'net> EmbassyTcpLink<'buf, 'net> {
    pub(crate) fn new(
        socket: TcpSocket<'buf, 'net>,
        mtu: u16,
        idx: usize,
        pool: &'buf RefCell<dyn BufferPoolDrop>,
    ) -> Self {
        Self {
            socket,
            mtu,
            idx,
            pool,
        }
    }
}

impl Drop for EmbassyTcpLink<'_, '_> {
    fn drop(&mut self) {
        self.pool.borrow_mut().release(self.idx);
    }
}

pub struct EmbassyTcpLinkTx<'buf, 'net> {
    socket: TcpWriter<'buf, 'net>,
    mtu: u16,
}

pub struct EmbassyTcpLinkRx<'buf, 'net> {
    socket: TcpReader<'buf, 'net>,
    mtu: u16,
}

impl<'buf, 'net> ZLinkInfo for EmbassyTcpLink<'buf, 'net> {
    fn mtu(&self) -> u16 {
        self.mtu
    }

    fn is_streamed(&self) -> bool {
        true
    }
}

impl<'buf, 'net> ZLinkInfo for EmbassyTcpLinkTx<'buf, 'net> {
    fn mtu(&self) -> u16 {
        self.mtu
    }

    fn is_streamed(&self) -> bool {
        true
    }
}

impl<'buf, 'net> ZLinkInfo for EmbassyTcpLinkRx<'buf, 'net> {
    fn mtu(&self) -> u16 {
        self.mtu
    }

    fn is_streamed(&self) -> bool {
        true
    }
}

impl<'buf, 'net> ZLinkTx for EmbassyTcpLink<'buf, 'net> {
    async fn write_all(&mut self, buffer: &[u8]) -> core::result::Result<(), LinkError> {
        self.socket
            .write_all(buffer)
            .await
            .map_err(|_| LinkError::LinkTxFailed)
    }
}

impl<'buf, 'net> ZLinkTx for EmbassyTcpLinkTx<'buf, 'net> {
    async fn write_all(&mut self, buffer: &[u8]) -> core::result::Result<(), LinkError> {
        self.socket
            .write_all(buffer)
            .await
            .map_err(|_| LinkError::LinkTxFailed)
    }
}

impl<'buf, 'net> ZLinkRx for EmbassyTcpLink<'buf, 'net> {
    async fn read(&mut self, buffer: &mut [u8]) -> core::result::Result<usize, LinkError> {
        self.socket
            .read(buffer)
            .await
            .map_err(|_| LinkError::LinkRxFailed)
    }

    async fn read_exact(&mut self, buffer: &mut [u8]) -> core::result::Result<(), LinkError> {
        self.socket
            .read_exact(buffer)
            .await
            .map_err(|_| LinkError::LinkRxFailed)
    }
}

impl<'buf, 'net> ZLinkRx for EmbassyTcpLinkRx<'buf, 'net> {
    async fn read(&mut self, buffer: &mut [u8]) -> core::result::Result<usize, LinkError> {
        self.socket
            .read(buffer)
            .await
            .map_err(|_| LinkError::LinkRxFailed)
    }

    async fn read_exact(&mut self, buffer: &mut [u8]) -> core::result::Result<(), LinkError> {
        self.socket
            .read_exact(buffer)
            .await
            .map_err(|_| LinkError::LinkRxFailed)
    }
}

impl<'buf, 'net> ZLink<'net> for EmbassyTcpLink<'buf, 'net> {
    type Tx<'a>
        = EmbassyTcpLinkTx<'a, 'net>
    where
        Self: 'a;

    type Rx<'a>
        = EmbassyTcpLinkRx<'a, 'net>
    where
        Self: 'a;

    fn split(&mut self) -> (Self::Tx<'_>, Self::Rx<'_>) {
        let (rx, tx) = self.socket.split();
        let tx = EmbassyTcpLinkTx {
            socket: tx,
            mtu: self.mtu,
        };
        let rx = EmbassyTcpLinkRx {
            socket: rx,
            mtu: self.mtu,
        };
        (tx, rx)
    }
}
