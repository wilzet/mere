//! Minimal colored logging macros for MeRe engine (build-time and runtime prints).

pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const BLUE: &str = "\x1b[34m";
pub const YELLOW: &str = "\x1b[33m";
pub const RESET: &str = "\x1b[0m";

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        println!("cargo:warning=\r  {}[MeRe: info]{}: {}", $crate::BLUE, $crate::RESET, format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => {{
        println!("cargo:warning=\r  {}[MeRe: success]{}: {}", $crate::GREEN, $crate::RESET, format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {{
        println!("cargo:warning=\r  {}[MeRe: warning]{}: {}", $crate::YELLOW, $crate::RESET, format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! error {
    (return $arg:expr) => {{
        $crate::error!("{}", $arg);
        return Err($arg.into());
    }};
    ($($arg:tt)*) => {{
        println!("cargo:warning=\r  {}[MeRe: error]{}: {}", $crate::RED, $crate::RESET, format_args!($($arg)*));
    }};
}
