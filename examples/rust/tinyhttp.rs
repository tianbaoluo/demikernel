use anyhow::Result;
use demikernel::{
  demi_sgarray_t,
  runtime::types::{demi_opcode_t, demi_qresult_t},
  LibOS, LibOSName, QDesc, QToken,
};
use hashbrown::{HashMap, HashSet};
use std::{
  net::SocketAddr,
  slice,
  time::{Duration, Instant},
};

// #[cfg(target_os = "linux")]
pub const AF_INET: i32 = libc::AF_INET;

// #[cfg(target_os = "linux")]
pub const SOCK_STREAM: i32 = libc::SOCK_STREAM;

const TIMEOUT_SECONDS: Duration = Duration::from_secs(256);

//======================================================================================================================
// Structures
//======================================================================================================================

pub struct TcpEchoServer {
  libos: LibOS,
  listening_sockqd: QDesc,
  connected_clients: HashSet<QDesc>,
  pending_qtokens: Vec<QToken>,
  pending_qtokens_reverse: HashMap<QToken, QDesc>,
  resp: demi_sgarray_t,
}

impl TcpEchoServer {
  pub fn new(mut libos: LibOS, local: SocketAddr) -> Result<Self> {
    let listening_sockqd: QDesc = libos.socket(AF_INET, SOCK_STREAM, 0)?;

    if let Err(e) = libos.bind(listening_sockqd, local) {
      println!("ERROR: {:?}", e);
      libos.close(listening_sockqd)?;
      anyhow::bail!("{:?}", e);
    }

    if let Err(e) = libos.listen(listening_sockqd, 1024) {
      println!("ERROR: {:?}", e);
      libos.close(listening_sockqd)?;
      anyhow::bail!("{:?}", e);
    }

    println!("INFO: listening on {:?} qd={:?}", local, listening_sockqd);

    let resp = mksga(&mut libos, b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n")?;

    return Ok(Self {
      libos,
      listening_sockqd,
      connected_clients: HashSet::default(),
      pending_qtokens: Vec::default(),
      pending_qtokens_reverse: HashMap::default(),
      resp,
    });
  }

  pub fn run(&mut self, log_interval: Option<u64>) -> Result<()> {
    let mut last_log: Instant = Instant::now();

    // Accept first connection.
    {
      let qt: QToken = self.libos.accept(self.listening_sockqd)?;
      let qr: demi_qresult_t = self.libos.wait(qt, Some(TIMEOUT_SECONDS))?;
      if qr.qr_opcode != demi_opcode_t::DEMI_OPC_ACCEPT {
        anyhow::bail!("failed to accept connection")
      }
      self.handle_accept(&qr)?;
    }

    loop {
      // if self.connected_clients.len() == 0 {
      //   println!("INFO: stopping...");
      //   break;
      // }

      // Dump statistics.
      if let Some(log_interval) = log_interval {
        if last_log.elapsed() > Duration::from_secs(log_interval) {
          // println!("INFO: {:?} clients connected", self.connected_clients.len(),);
          last_log = Instant::now();
        }
      }

      // println!("wait tokens={:?}", self.pending_qtokens);
      // println!("wait pending_qtokens_reverse={:?}", self.pending_qtokens_reverse);
      let qr: demi_qresult_t = {
        let (index, qr): (usize, demi_qresult_t) =
          self.libos.wait_any(&self.pending_qtokens, Some(TIMEOUT_SECONDS))?;
        self.unregister_operation(index)?;
        qr
      };

      // println!("wait-result op={:?} qd={} ret={}", qr.qr_opcode, qr.qr_qd, qr.qr_ret);
      match qr.qr_opcode {
        demi_opcode_t::DEMI_OPC_ACCEPT => self.handle_accept(&qr)?,
        demi_opcode_t::DEMI_OPC_POP => self.handle_pop(&qr)?,
        demi_opcode_t::DEMI_OPC_PUSH => self.handle_push(&qr)?,
        demi_opcode_t::DEMI_OPC_FAILED => self.handle_fail(&qr)?,
        demi_opcode_t::DEMI_OPC_INVALID => self.handle_unexpected("invalid", &qr)?,
        demi_opcode_t::DEMI_OPC_CLOSE => self.handle_unexpected("close", &qr)?,
        demi_opcode_t::DEMI_OPC_CONNECT => self.handle_unexpected("connect", &qr)?,
      }
    }

    Ok(())
  }

  fn issue_accept(&mut self) -> Result<()> {
    let qt: QToken = self.libos.accept(self.listening_sockqd)?;
    self.register_operation(self.listening_sockqd, qt);
    Ok(())
  }

  fn issue_push(&mut self, qd: QDesc, sga: &demi_sgarray_t) -> Result<()> {
    let qt: QToken = self.libos.push(qd, &sga)?;
    self.register_operation(qd, qt);
    Ok(())
  }

  fn issue_resp(&mut self, qd: QDesc) -> Result<()> {
    let qt: QToken = self.libos.push(qd, &self.resp)?;
    self.register_operation(qd, qt);
    Ok(())
  }

  fn issue_pop(&mut self, qd: QDesc) -> Result<()> {
    let qt: QToken = self.libos.pop(qd, None)?;
    self.register_operation(qd, qt);
    Ok(())
  }

  fn handle_fail(&mut self, qr: &demi_qresult_t) -> Result<()> {
    let qd: QDesc = qr.qr_qd.into();
    let qt: QToken = qr.qr_qt.into();
    let errno: i64 = qr.qr_ret;

    if is_closed(errno) {
      // println!("{:?}/{:?} got errno={} isclosed?", qd, qt, errno);
      self.handle_close(qd)?;
    } else {
      println!(
        "WARN: operation failed, ignoring (qd={:?}, qt={:?}, errno={:?})",
        qd, qt, errno
      );
      // println!("wait pending_qtokens_reverse={:?}", self.pending_qtokens_reverse);
      self.issue_accept()?;
    }

    Ok(())
  }

  fn handle_push(&mut self, qr: &demi_qresult_t) -> Result<()> {
    // println!("hadle-push: {}", qr.qr_qd);
    // self.libos.close(qr.qr_qd.into())?;
    // let qd: QDesc = qr.qr_qd.into();
    // self.issue_pop(qd)?;
    // // self.handle_close(qd)?;
    // // let qd: QDesc = qr.qr_qd.into();
    // let qt = self.libos.async_close(qd)?;
    // self.register_operation(qd, qt);
    Ok(())
  }

  fn handle_unexpected(&mut self, op_name: &str, qr: &demi_qresult_t) -> Result<()> {
    let qd: QDesc = qr.qr_qd.into();
    let qt: QToken = qr.qr_qt.into();
    println!(
      "WARN: unexpected {} operation completed, ignoring (qd={:?}, qt={:?})",
      op_name, qd, qt
    );
    Ok(())
  }

  fn handle_accept(&mut self, qr: &demi_qresult_t) -> Result<()> {
    let new_qd: QDesc = unsafe { qr.qr_value.ares.qd.into() };
    self.connected_clients.insert(new_qd);
    println!(
      "INFO: {:?} clients connected #tokens={}",
      self.connected_clients.len(),
      self.pending_qtokens.len()
    );
    self.issue_pop(new_qd)?;
    self.issue_accept()?;
    Ok(())
  }

  fn handle_pop(&mut self, qr: &demi_qresult_t) -> Result<()> {
    let qd: QDesc = qr.qr_qd.into();
    let sga: demi_sgarray_t = unsafe { qr.qr_value.sga };

    // Check if we received any data.
    if sga.sga_segs[0].sgaseg_len == 0 {
      println!("INFO: client closed connection (qd={:?})", qd);
      self.handle_close(qd)?;
    } else {
      let ptr: *mut u8 = sga.sga_segs[0].sgaseg_buf as *mut u8;
      let len: usize = sga.sga_segs[0].sgaseg_len as usize;
      let slice: &[u8] = unsafe { slice::from_raw_parts_mut(ptr, len) };
      println!("bytes #{}\t{:?}", len, String::from_utf8_lossy(slice));
      // self.issue_push2(qd)?;
      self.issue_resp(qd)?;
      // Pop more data.
      self.issue_pop(qd)?;
      // let qt = self.libos.async_close(qd)?;
      // self.register_operation(qd, qt);
      // self.handle_close(qd)?;
    }

    self.libos.sgafree(sga)?;
    Ok(())
  }

  fn handle_close(&mut self, qd: QDesc) -> Result<()> {
    // println!("close qd {:?}", qd);
    let qts_drained: HashMap<QToken, QDesc> = self.pending_qtokens_reverse.extract_if(|_k, v| v == &qd).collect();
    // let _: Vec<_> = self
    //   .pending_qtokens
    //   .extract_if(|x| qts_drained.contains_key(x))
    //   .collect();
    self.pending_qtokens.retain(|x| !qts_drained.contains_key(x));
    self.connected_clients.remove(&qd);
    // println!("pending_qtokens_reverse: {:?}", self.pending_qtokens_reverse);
    // println!("pending_qtokens: {:?}", self.pending_qtokens);
    // println!("connected_clients: {:?}", self.connected_clients);
    self.libos.close(qd)?;
    Ok(())
  }

  fn register_operation(&mut self, qd: QDesc, qt: QToken) {
    self.pending_qtokens_reverse.insert(qt, qd);
    self.pending_qtokens.push(qt);
  }

  fn unregister_operation(&mut self, index: usize) -> Result<()> {
    let qt: QToken = self.pending_qtokens.remove(index);
    self.pending_qtokens_reverse
      .remove(&qt)
      .ok_or(anyhow::anyhow!("unregistered queue token"))?;
    Ok(())
  }
}

impl Drop for TcpEchoServer {
  fn drop(&mut self) {
    for qd in self.connected_clients.drain().collect::<Vec<_>>() {
      if let Err(e) = self.handle_close(qd) {
        println!("ERROR: close() failed (error={:?}", e);
        println!("WARN: leaking qd={:?}", qd);
      }
    }
    if let Err(e) = self.handle_close(self.listening_sockqd) {
      println!("ERROR: {:?}", e);
    }
  }
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

fn is_closed(ret: i64) -> bool {
  match ret as i32 {
    libc::ECONNRESET | libc::ENOTCONN | libc::ECANCELED | libc::EBADF => true,
    _ => false,
  }
}

fn main() -> anyhow::Result<()> {
  // std::env::set_var("RUST_LOG", "trace");
  std::env::set_var("CONFIG_PATH", "/Users/mr00027ml/work/sandbox/demikernel/config.yaml");
  let libos: LibOS = match LibOS::new(LibOSName::Catnap, None) {
    Ok(libos) => libos,
    Err(e) => anyhow::bail!("failed to initialize libos: {:?}", e.cause),
  };
  let addr = "192.168.104.77:3456".parse().unwrap();
  
  // linux dpdk
  // std::env::set_var("DPDK_PROT", "TCP");
  // std::env::set_var("RUN_CPU", "7");
  // std::env::set_var("CONFIG_PATH", "/root/work/demikernel/hft_config.yaml");
  // let libos: LibOS = match LibOS::new(LibOSName::Catnip, None) {
  //   Ok(libos) => libos,
  //   Err(e) => anyhow::bail!("failed to initialize libos: {:?}", e.cause),
  // };
  // let addr = "10.132.6.131:1234".parse().unwrap();
  
  let mut server: TcpEchoServer = TcpEchoServer::new(libos, addr)?;
  server.run(Some(2))?;
  Ok(())
}
