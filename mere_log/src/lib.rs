#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        const BLUE: &str = "\x1b[34m";
        const RESET: &str = "\x1b[0m";
        println!("cargo:warning=\r  {}[MeRe: info]{}: {}", BLUE, RESET, format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => {{
        const GREEN: &str = "\x1b[32m";
        const RESET: &str = "\x1b[0m";
        println!("cargo:warning=\r  {}[MeRe: success]{}: {}", GREEN, RESET, format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {{
        const YELLOW: &str = "\x1b[33m";
        const RESET: &str = "\x1b[0m";
        println!("cargo:warning=\r  {}[MeRe: warning]{}: {}", YELLOW, RESET, format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        const RED: &str = "\x1b[31m";
        const RESET: &str = "\x1b[0m";
        println!("cargo:warning=\r  {}[MeRe: error]{}: {}", RED, RESET, format!($($arg)*));
    }};
}
