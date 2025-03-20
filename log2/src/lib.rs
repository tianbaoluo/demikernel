

#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => (
        log::error!($($arg)+)
    )
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => (
        log::warn!($($arg)+)
    )
}

#[cfg(feature = "info")]
#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => (
        log::info!($($arg)+)
    )
}

#[cfg(not(feature = "info"))]
#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => {()}
}

#[cfg(feature = "debug")]
#[macro_export]
macro_rules! debug {
    ($($arg:tt)+) => (
        log::debug!($($arg)+)
    )
}


#[cfg(not(feature = "debug"))]
#[macro_export]
macro_rules! debug {
    ($($arg:tt)+) => {()}
}

#[cfg(feature = "debug")]
#[macro_export]
macro_rules! trace {
    ($($arg:tt)+) => (
        log::trace!($($arg)+)
    )
}


#[cfg(not(feature = "debug"))]
#[macro_export]
macro_rules! trace {
    ($($arg:tt)+) => {()}
}