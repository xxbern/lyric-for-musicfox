pub mod udp_send;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Toggle,
    Next,
    Previous,
    Like,
    Seek { position_ms: i64 },
    Volume { level: i32 },
}

pub use udp_send::send_command;
