macro_rules! debug {
    ( $( $t:tt )* ) => { if crate::log::do_debug() { eprintln!($( $t )* ) } }
}

pub fn do_debug() -> bool {
    std::env::var_os("CLIP_OTP_DEBUG").is_some()
}
