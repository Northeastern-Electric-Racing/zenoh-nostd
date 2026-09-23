use embassy_net::udp::{UdpMetadata, UdpSocket};
use zenoh_nostd::platform::*;

pub struct EmbassyUdpLink<'net> {
    socket: UdpSocket<'net>,
    addr: UdpMetadata,
    mtu: u16,
}

impl<'net> EmbassyUdpLink<'net> {
    pub(crate) fn new(socket: UdpSocket<'net>, metadata: UdpMetadata, mtu: u16) -> Self {
        Self {
            socket,
            addr: metadata,
            mtu,
        }
    }
}

pub struct EmbassyUdpLinkTx<'buf, 'net> {
    socket: &'buf UdpSocket<'net>,
    mtu: u16,
    addr: UdpMetadata,
}

pub struct EmbassyUdpLinkRx<'buf, 'net> {
    socket: &'buf UdpSocket<'net>,
    mtu: u16,
}

impl<'net> ZLinkInfo for EmbassyUdpLink<'net> {
    fn mtu(&self) -> u16 {
        self.mtu
    }

    fn is_streamed(&self) -> bool {
        false
    }
}

impl<'buf, 'net> ZLinkInfo for EmbassyUdpLinkTx<'buf, 'net> {
    fn mtu(&self) -> u16 {
        self.mtu
    }

    fn is_streamed(&self) -> bool {
        false
    }
}

impl<'buf, 'net> ZLinkInfo for EmbassyUdpLinkRx<'buf, 'net> {
    fn mtu(&self) -> u16 {
        self.mtu
    }

    fn is_streamed(&self) -> bool {
        false
    }
}

impl<'net> ZLinkTx for EmbassyUdpLink<'net> {
    async fn write_all(&mut self, buffer: &[u8]) -> core::result::Result<(), LinkError> {
        self.socket
            .send_to(buffer, self.addr)
            .await
            .map_err(|_| LinkError::LinkTxFailed)
    }
}

impl<'buf, 'net> ZLinkTx for EmbassyUdpLinkTx<'buf, 'net> {
    async fn write_all(&mut self, buffer: &[u8]) -> core::result::Result<(), LinkError> {
        self.socket
            .send_to(buffer, self.addr)
            .await
            .map_err(|_| LinkError::LinkTxFailed)
    }
}

impl<'net> ZLinkRx for EmbassyUdpLink<'net> {
    async fn read(&mut self, buffer: &mut [u8]) -> core::result::Result<usize, LinkError> {
        self.socket
            .recv_from(buffer)
            .await
            .map_err(|_| LinkError::LinkRxFailed)
            .map(|m| m.0)
    }

    async fn read_exact(&mut self, buffer: &mut [u8]) -> core::result::Result<(), LinkError> {
        self.socket
            .recv_from(buffer)
            .await
            .map_err(|_| LinkError::LinkRxFailed)
            .map(|_| ())
    }
}

impl<'buf, 'net> ZLinkRx for EmbassyUdpLinkRx<'buf, 'net> {
    async fn read(&mut self, buffer: &mut [u8]) -> core::result::Result<usize, LinkError> {
        self.socket
            .recv_from(buffer)
            .await
            .map_err(|_| LinkError::LinkRxFailed)
            .map(|m| m.0)
    }

    async fn read_exact(&mut self, buffer: &mut [u8]) -> core::result::Result<(), LinkError> {
        self.socket
            .recv_from(buffer)
            .await
            .map_err(|_| LinkError::LinkRxFailed)
            .map(|_| ())
    }
}

impl<'net> ZLink<'net> for EmbassyUdpLink<'net> {
    type Tx<'buf>
        = EmbassyUdpLinkTx<'buf, 'net>
    where
        Self: 'buf;

    type Rx<'buf>
        = EmbassyUdpLinkRx<'buf, 'net>
    where
        Self: 'buf;

    fn split(&mut self) -> (Self::Tx<'_>, Self::Rx<'_>) {
        let tx = EmbassyUdpLinkTx {
            socket: &self.socket,
            mtu: self.mtu,
            addr: self.addr,
        };
        let rx = EmbassyUdpLinkRx {
            socket: &self.socket,
            mtu: self.mtu,
        };
        (tx, rx)
    }
}
