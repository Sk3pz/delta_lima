use std::fmt;
use better_term::Color;

const LOG_BRACKET_COLOR: Color = Color::BrightBlack;
const LOG_POINT_COLOR: Color = Color::White;
const LOG_MSG_COLOR: Color = Color::BrightWhite;
const DEBUG_COLOR: Color = Color::BrightBlack;
const INFO_COLOR: Color = Color::Cyan;
const WARN_COLOR: Color = Color::BrightYellow;
const ERROR_COLOR: Color = Color::BrightRed;

pub fn _debug(args: fmt::Arguments) {
    println!("{LOG_BRACKET_COLOR}[{DEBUG_COLOR}DBG{LOG_BRACKET_COLOR}] {LOG_POINT_COLOR}> {LOG_MSG_COLOR}{}", args);
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => ($crate::logging::_debug(format_args!($($arg)*)));
}

pub fn _info(args: fmt::Arguments) {
    println!("{LOG_BRACKET_COLOR}[{INFO_COLOR}INF{LOG_BRACKET_COLOR}] {LOG_POINT_COLOR}> {LOG_MSG_COLOR}{}", args);
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => ($crate::logging::_info(format_args!($($arg)*)));
}

pub fn _warn(args: fmt::Arguments) {
    println!("{LOG_BRACKET_COLOR}[{WARN_COLOR}WRN{LOG_BRACKET_COLOR}] {LOG_POINT_COLOR}> {LOG_MSG_COLOR}{}", args);
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => ($crate::logging::_warn(format_args!($($arg)*)));
}

pub fn _error(args: fmt::Arguments) {
    println!("{LOG_BRACKET_COLOR}[{ERROR_COLOR}ERR{LOG_BRACKET_COLOR}] {LOG_POINT_COLOR}> {LOG_MSG_COLOR}{}", args);
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => ($crate::logging::_error(format_args!($($arg)*)));
}