use std::net::{SocketAddr, UdpSocket};
use std::ptr::slice_from_raw_parts;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
  let dests: Vec<SocketAddr> = vec![
    "192.168.104.77:17070".parse().unwrap(),
    // "10.132.6.150:17070".parse().unwrap(),
    // "10.132.6.131:17070".parse().unwrap(),
  ];

  let udp = UdpSocket::bind("192.168.104.77:0")?;
  loop {
    for dest in dests.iter() {
      let now = now_us();
      udp.send_to(i64_to_bytes(&now), dest)?;
    }
    std::thread::sleep(Duration::from_secs(3));
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
