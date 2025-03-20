
use ::anyhow::Result;
use ::demikernel::{demi_sgarray_t, runtime::types::demi_opcode_t, LibOS, LibOSName, QDesc, QToken};
use ::std::mem;
use ::std::{net::{Ipv4Addr, SocketAddr, SocketAddrV4}, time::Duration};
use std::slice;

const TIMEOUT_SECONDS: Duration = Duration::from_secs(5);

struct Application {
  libos: LibOS,
  sockqd: QDesc,
  pfs_endpoint: SocketAddr,
  heartbeat: demi_sgarray_t,
}

impl Application {
  pub fn new(mut libos: LibOS, local_socket_addr: SocketAddr, pfs_endpoint: SocketAddr, heartbeat: &[u8]) -> Result<Self> {
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

    let heartbeat = mksga(&mut libos, heartbeat)?;
    Ok(Self { libos, sockqd, pfs_endpoint, heartbeat })
  }

  pub fn run(&mut self) -> Result<()> {
    let mut qtokens: Vec<QToken> = Vec::new();

    // Pop first packet.
    let qt: QToken = match self.libos.pop(self.sockqd, None) {
      Ok(qt) => qt,
      Err(e) => anyhow::bail!("failed to pop data from socket: {:?}", e),
    };
    qtokens.push(qt);

    let mut inlight_send = false;
    loop {
      match self.libos.wait_any(&qtokens, Some(TIMEOUT_SECONDS)) {
        Ok((i, qr)) => {
          qtokens.swap_remove(i);

          // Parse result.
          match qr.qr_opcode {
            // Pop completed.
            demi_opcode_t::DEMI_OPC_POP => {
              // let sockqd: QDesc = qr.qr_qd.into();
              let sga: demi_sgarray_t = unsafe { qr.qr_value.sga };
              let saddr: SocketAddr = match sockaddr_to_socketaddrv4(&unsafe { qr.qr_value.sga.sga_addr }) {
                Ok(saddr) => SocketAddr::from(saddr),
                Err(e) => {
                  // If error, free scatter-gather array.
                  if let Err(e) = self.libos.sgafree(sga) {
                    println!("ERROR: sgafree() failed (error={:?})", e);
                    println!("WARN: leaking sga");
                  };
                  anyhow::bail!("could not parse sockaddr: {}", e)
                },
              };

              let ptr: *mut u8 = sga.sga_segs[0].sgaseg_buf as *mut u8;
              let len: usize = sga.sga_segs[0].sgaseg_len as usize;
              let slice: &[u8] = unsafe { slice::from_raw_parts_mut(ptr, len) };
              println!("bytes #{} from {}\t{:?}", len, saddr, slice);

              let qt: QToken = match self.libos.pop(self.sockqd, None) {
                Ok(qt) => qt,
                Err(e) => anyhow::bail!("failed to pop data from socket: {:?}", e),
              };
              qtokens.push(qt);
            },
            // Push completed.
            demi_opcode_t::DEMI_OPC_PUSH => {
              println!("send done");
              inlight_send = false;
            },
            demi_opcode_t::DEMI_OPC_FAILED => anyhow::bail!("operation failed"),
            _ => anyhow::bail!("unexpected result"),
          };
        },
        Err(e) => {
          println!("wait-any error: inlight_send={} errno={}/{:?}", inlight_send, e.errno, e);
          if !inlight_send && (e.errno == libc::ETIMEDOUT || e.errno == 110) {
            // send heartbeat
            let qt: QToken = match self.libos.pushto(self.sockqd, &self.heartbeat, self.pfs_endpoint) {
              Ok(qt) => qt,
              Err(e) => anyhow::bail!("failed to push data to socket: {:?}", e),
            };
            qtokens.push(qt);
            inlight_send = true;
          } else {
            eprintln!("got error: {:?}", e);
          }
        },
      }
    }
  }
}

impl Drop for Application {
  fn drop(&mut self) {
    if let Err(e) = self.libos.sgafree(self.heartbeat) {
      println!("ERROR: sgafree() failed (error={:?}", e);
    }
    if let Err(e) = self.libos.close(self.sockqd) {
      println!("ERROR: close() failed (error={:?}", e);
      println!("WARN: leaking sockqd={:?}", self.sockqd);
    }
  }
}

fn sockaddr_to_socketaddrv4(saddr: *const libc::sockaddr) -> Result<SocketAddrV4> {
  let sin: libc::sockaddr_in = unsafe { *mem::transmute::<*const libc::sockaddr, *const libc::sockaddr_in>(saddr) };
  if sin.sin_family != libc::AF_INET as _ {
    anyhow::bail!("communication domain not supported");
  };
  let addr: Ipv4Addr = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
  let port: u16 = u16::from_be(sin.sin_port);
  Ok(SocketAddrV4::new(addr, port))
}

fn mksga(libos: &mut LibOS, data: &[u8]) -> Result<demi_sgarray_t> {
  let size = data.len();
  let sga: demi_sgarray_t = match libos.sgaalloc(size) {
    Ok(sga) => sga,
    Err(e) => anyhow::bail!("failed to allocate scatter-gather array: {:?}", e),
  };

  // Ensure that allocated array has the requested size.
  if sga.sga_segs[0].sgaseg_len as usize != size {
    if let Err(e) = libos.sgafree(sga) {
      println!("ERROR: sgafree() failed (error={:?})", e);
      println!("WARN: leaking sga");
    };
    let seglen: usize = sga.sga_segs[0].sgaseg_len as usize;
    anyhow::bail!(
            "failed to allocate scatter-gather array: expected size={:?} allocated size={:?}",
            size,
            seglen
        );
  }
  // Fill in the array.
  let ptr: *mut u8 = sga.sga_segs[0].sgaseg_buf as *mut u8;
  let len: usize = sga.sga_segs[0].sgaseg_len as usize;
  let slice: &mut [u8] = unsafe { slice::from_raw_parts_mut(ptr, len) };
  slice.copy_from_slice(data);

  Ok(sga)
}

fn main() -> Result<()> {
  demikernel::runtime::logging::initialize();

  // linux dpdk
  std::env::set_var("DPDK_PROT", "UDP");
  std::env::set_var("RUN_CPU", "6");
  let libos: LibOS = match LibOS::new(LibOSName::Catnip, None) {
    Ok(libos) => libos,
    Err(e) => anyhow::bail!("failed to initialize libos: {:?}", e.cause),
  };

  let local_addr: SocketAddr = "10.34.8.241:17070".parse().unwrap();
  let pfs_endpoint: SocketAddr = "10.89.26.2:8000".parse().unwrap();
  let heartbeat = b"A 10.34.8.241:17070 pfs_bybit_snowball:REDUSDT pfs_bybit_snowball:ETHUSDT pfs_bybit_snowball:BTCUSDT";
  Application::new(libos, local_addr, pfs_endpoint, heartbeat)?.run()
}
