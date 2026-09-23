#![no_std]

use core::{cell::RefCell, net::SocketAddr};
use embassy_net::{
    Stack,
    tcp::TcpSocket,
    udp::UdpSocket,
    wire::{IpAddress, IpEndpoint},
};
use zenoh_nostd::platform::*;

pub mod tcp;
pub mod udp;

struct BufferPool<T, const MTU: usize, const SOCKS: usize> {
    tx_buffers: [[T; MTU]; SOCKS],
    rx_buffers: [[T; MTU]; SOCKS],
    used: [bool; SOCKS],
}

impl<T, const MTU: usize, const SOCKS: usize> BufferPool<T, MTU, SOCKS> {
    fn new(value: T) -> Self
    where
        T: Copy,
    {
        Self {
            tx_buffers: [[value; MTU]; SOCKS],
            rx_buffers: [[value; MTU]; SOCKS],
            used: [false; SOCKS],
        }
    }

    fn allocate(&mut self) -> Option<usize> {
        self.used.iter().position(|&used| !used).inspect(|&idx| {
            self.used[idx] = true;
        })
    }
}

pub(crate) trait BufferPoolDrop {
    fn release(&mut self, idx: usize);
}

impl<T, const MTU: usize, const SOCKS: usize> BufferPoolDrop for BufferPool<T, MTU, SOCKS> {
    fn release(&mut self, idx: usize) {
        if idx < SOCKS {
            self.used[idx] = false;
        }
    }
}

pub struct EmbassyLinkManager<'net, const MTU: usize, const SOCKS: usize> {
    stack: Stack<'net>,
    buffers: RefCell<BufferPool<u8, MTU, SOCKS>>,
}

impl<'net, const MTU: usize, const SOCKS: usize> EmbassyLinkManager<'net, MTU, SOCKS> {
    pub fn new(stack: Stack<'net>) -> Self {
        Self {
            stack,
            buffers: RefCell::new(BufferPool::new(0)),
        }
    }

    #[allow(clippy::mut_from_ref)]
    fn allocate_buffers(&self) -> Option<(usize, &mut [u8], &mut [u8])> {
        let idx = self.buffers.borrow_mut().allocate()?;

        // SAFETY: This pool is simple, I should not have made any mistake. The reference will still be valid
        // because Tcp borrows EmbassyLinkManager and so EmbassyLinkManager can't be moved.
        let buffers = unsafe { &mut *self.buffers.as_ptr() };
        let tx = &mut buffers.tx_buffers[idx];
        let rx = &mut buffers.rx_buffers[idx];

        Some((idx, tx, rx))
    }
}

#[derive(ZLinkInfo, ZLinkTx, ZLinkRx, ZLink)]
#[zenoh(ZLink = (EmbassyLinkTx<'link, 'net>, EmbassyLinkRx<'link, 'net>))]
pub enum EmbassyLink<'buf, 'net> {
    Tcp(tcp::EmbassyTcpLink<'buf, 'net>),
    Udp(udp::EmbassyUdpLink<'net>),
}

#[derive(ZLinkInfo, ZLinkTx)]
pub enum EmbassyLinkTx<'link, 'net> {
    Tcp(tcp::EmbassyTcpLinkTx<'link, 'net>),
    Udp(udp::EmbassyUdpLinkTx<'link, 'net>),
}

#[derive(ZLinkInfo, ZLinkRx)]
pub enum EmbassyLinkRx<'link, 'net> {
    Tcp(tcp::EmbassyTcpLinkRx<'link, 'net>),
    Udp(udp::EmbassyUdpLinkRx<'link, 'net>),
}

impl<'net, const MTU: usize, const SOCKS: usize> ZLinkManager<'net>
    for EmbassyLinkManager<'net, MTU, SOCKS>
{
    type Link<'a>
        = EmbassyLink<'a, 'net>
    where
        Self: 'a;

    async fn connect(
        &self,
        endpoint: Endpoint<'_>,
    ) -> core::result::Result<Self::Link<'_>, LinkError> {
        let protocol = endpoint.protocol();
        let address = endpoint.address();

        match protocol.as_str() {
            "tcp" => {
                let dst_addr = SocketAddr::try_from(address)?;
                let (idx, tx, rx) = self.allocate_buffers().ok_or(LinkError::CouldNotConnect)?;
                let mut socket =
                    TcpSocket::new(self.stack, rx, tx).map_err(|_| LinkError::CouldNotConnect)?;

                let address: IpAddress = match dst_addr.ip() {
                    core::net::IpAddr::V4(v4) => IpAddress::Ipv4(v4),
                    core::net::IpAddr::V6(_) => {
                        zenoh::zbail!(LinkError::CouldNotConnect)
                    }
                };

                let ip_endpoint = IpEndpoint::new(address, dst_addr.port());

                socket
                    .connect(ip_endpoint)
                    .await
                    .map_err(|_| LinkError::CouldNotConnect)?;

                Ok(Self::Link::Tcp(tcp::EmbassyTcpLink::new(
                    socket,
                    MTU as u16,
                    idx,
                    &self.buffers,
                )))
            }
            "udp" => {
                let dst_addr = SocketAddr::try_from(address)?;

                let mut socket =
                    UdpSocket::new(self.stack).map_err(|_| LinkError::CouldNotConnect)?;
                socket.bind(0).map_err(|_| LinkError::CouldNotConnect)?;

                let address: IpAddress = match dst_addr.ip() {
                    core::net::IpAddr::V4(v4) => IpAddress::Ipv4(v4),
                    core::net::IpAddr::V6(_) => {
                        zenoh::zbail!(LinkError::CouldNotConnect)
                    }
                };

                let ip_endpoint = IpEndpoint::new(address, dst_addr.port());

                Ok(Self::Link::Udp(udp::EmbassyUdpLink::new(
                    socket,
                    ip_endpoint.into(),
                    MTU as u16,
                )))
            }
            _ => zenoh::zbail!(LinkError::CouldNotParseProtocol),
        }
    }

    async fn listen(
        &self,
        endpoint: Endpoint<'_>,
    ) -> core::result::Result<Self::Link<'_>, LinkError> {
        let protocol = endpoint.protocol();
        let address = endpoint.address();

        match protocol.as_str() {
            "tcp" => {
                let src_addr = SocketAddr::try_from(address)?;
                let (idx, tx, rx) = self.allocate_buffers().ok_or(LinkError::CouldNotConnect)?;
                let mut socket =
                    TcpSocket::new(self.stack, rx, tx).map_err(|_| LinkError::CouldNotConnect)?;

                let address: IpAddress = match src_addr.ip() {
                    core::net::IpAddr::V4(v4) => IpAddress::Ipv4(v4),
                    core::net::IpAddr::V6(_) => {
                        zenoh::zbail!(LinkError::CouldNotConnect)
                    }
                };

                let ip_endpoint = IpEndpoint::new(address, src_addr.port());

                socket
                    .connect(ip_endpoint)
                    .await
                    .map_err(|_| LinkError::CouldNotConnect)?;

                Ok(Self::Link::Tcp(tcp::EmbassyTcpLink::new(
                    socket,
                    MTU as u16,
                    idx,
                    &self.buffers,
                )))
            }
            _ => zenoh::zbail!(LinkError::CouldNotParseProtocol),
        }
    }
}
