
use ::anyhow::Result;
use ::demikernel::{demi_sgarray_t, runtime::types::demi_opcode_t, LibOS, LibOSName, QDesc, QToken};
use ::std::mem;
use ::std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    time::Duration,
};
use std::mem::transmute;
use std::ptr::slice_from_raw_parts;
use std::slice;

const POLL_MODE: Option<Duration> = Some(Duration::ZERO);

struct Application {
    libos: LibOS,
    sockqd: QDesc,
}

impl Application {
    pub fn new(mut libos: LibOS, local_socket_addr: SocketAddr) -> Result<Self> {
        let sockqd: QDesc = match libos.socket(libc::AF_INET, libc::SOCK_DGRAM, 0) {
            Ok(sockqd) => sockqd,
            Err(e) => anyhow::bail!("failed to create socket: {:?}", e),
        };

        match libos.bind(sockqd, local_socket_addr) {
            Ok(()) => (),
            Err(e) => {
                // If error, close socket.
                if let Err(e) = libos.close(sockqd) {
                    println!("ERROR: close() failed (error={:?}", e);
                    println!("WARN: leaking sockqd={:?}", sockqd);
                }
                anyhow::bail!("failed to bind socket: {:?}", e)
            },
        };

        println!("Local Address: {:?}", local_socket_addr);

        Ok(Self { libos, sockqd })
    }

    pub fn run(&mut self) -> Result<()> {
        let mut qtokens: Vec<QToken> = Vec::new();

        // Pop first packet.
        let qt: QToken = match self.libos.pop(self.sockqd, None) {
            Ok(qt) => qt,
            Err(e) => anyhow::bail!("failed to pop data from socket: {:?}", e),
        };
        qtokens.push(qt);

        loop {
            let (i, qr) = match self.libos.wait_any(&qtokens, POLL_MODE) {
                Ok((i, qr)) => (i, qr),
                Err(e) => {
                    if e.errno == libc::ETIMEDOUT {
                        continue;
                    }
                    anyhow::bail!("operation failed: {:?}", e)
                },
            };
            qtokens.swap_remove(i);

            // Parse result.
            match qr.qr_opcode {
                // Pop completed.
                demi_opcode_t::DEMI_OPC_POP => {
                    let recv_us = now_us();
                    let sockqd: QDesc = qr.qr_qd.into();
                    let sga: demi_sgarray_t = unsafe { qr.qr_value.sga };
                    
                    let ptr: *mut u8 = sga.sga_segs[0].sgaseg_buf as *mut u8;
                    let len: usize = sga.sga_segs[0].sgaseg_len as usize;
                    assert!(len >= 8);
                    let sent_us = read_i64(ptr);
                    println!("latency-us: {}", recv_us - sent_us);
                    if let Err(e) = self.libos.sgafree(sga) {
                        println!("ERROR: sgafree() failed (error={:?})", e);
                        println!("WARN: leaking sga");
                    }
                    let sockqd: QDesc = qr.qr_qd.into();
                    let qt: QToken = match self.libos.pop(sockqd, None) {
                        Ok(qt) => qt,
                        Err(e) => anyhow::bail!("failed to pop data from socket: {:?}", e),
                    };
                    qtokens.push(qt);
                },
                // Push completed.
                demi_opcode_t::DEMI_OPC_PUSH => {
                    // Pop another packet.

                },
                demi_opcode_t::DEMI_OPC_FAILED => anyhow::bail!("operation failed"),
                _ => anyhow::bail!("unexpected result"),
            };
        }
    }

    pub fn sockaddr_to_socketaddrv4(saddr: *const libc::sockaddr) -> Result<SocketAddrV4> {
        // TODO: Change the logic below and rename this function once we support V6 addresses as well.
        let sin: libc::sockaddr_in =
            unsafe { *mem::transmute::<*const libc::sockaddr, *const libc::sockaddr_in>(saddr) };
        if sin.sin_family != libc::AF_INET as _ {
            anyhow::bail!("communication domain not supported");
        };
        let addr: Ipv4Addr = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
        let port: u16 = u16::from_be(sin.sin_port);
        Ok(SocketAddrV4::new(addr, port))
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        if let Err(e) = self.libos.close(self.sockqd) {
            println!("ERROR: close() failed (error={:?}", e);
            println!("WARN: leaking sockqd={:?}", self.sockqd);
        }
    }
}

fn main() -> Result<()> {
    // std::env::set_var("CONFIG_PATH", "/Users/mr00027ml/work/sandbox/demikernel/config.yaml");
    // let libos: LibOS = match LibOS::new(LibOSName::Catnap, None) {
    //     Ok(libos) => libos,
    //     Err(e) => panic!("failed to initialize libos: {:?}", e),
    // };
    // let local_addr: SocketAddr = "192.168.104.77:17070".parse().unwrap();
    
    // linux dpdk
    // std::env::set_var("DPDK_PROT", "UDP");
    let use_dpdk = false;
    if use_dpdk {
        std::env::set_var("RUN_CPU", "6");
        std::env::set_var("CONFIG_PATH", "hft_config.yaml");
        let libos: LibOS = match LibOS::new(LibOSName::Catnip, None) {
            Ok(libos) => libos,
            Err(e) => anyhow::bail!("failed to initialize libos: {:?}", e.cause),
        };
        let local_addr: SocketAddr = "10.132.6.131:17070".parse().unwrap();

        Application::new(libos, local_addr)?.run()
    } else {
        std::env::set_var("RUN_CPU", "7");
        std::env::set_var("CONFIG_PATH", "config.yaml");
        let libos: LibOS = match LibOS::new(LibOSName::Catnap, None) {
            Ok(libos) => libos,
            Err(e) => anyhow::bail!("failed to initialize libos: {:?}", e.cause),
        };
        let local_addr: SocketAddr = "10.132.6.150:17070".parse().unwrap();

        Application::new(libos, local_addr)?.run()
    }
}

#[inline(always)]
fn read_i64(ptr: *const u8) -> i64 {
    unsafe {
        *(ptr as *const i64)
    }
}

#[inline(always)]
fn i64_to_bytes(val: &i64) -> &[u8] {
    unsafe {
        &*slice_from_raw_parts(val as *const i64 as *const u8, size_of::<i64>())
    }
}

#[inline(always)]
fn now_us() -> i64 {
    unsafe {
        let mut time: libc::timespec = std::mem::zeroed();
        if libc::clock_gettime(libc::CLOCK_REALTIME, &mut time) == -1 {
            unreachable!("Call libc::clock_gettime(libc::CLOCK_REALTIME, &mut time) error: {:?}", std::io::Error::last_os_error());
        }
        time.tv_sec as i64 * 1000_000 + time.tv_nsec / 1000
    }
}