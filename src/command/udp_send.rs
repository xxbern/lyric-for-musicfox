use super::Command;
use crate::error::AppResult;
use serde::Serialize;
use std::net::UdpSocket;
use std::sync::OnceLock;

static OUTBOUND_SOCKET: OnceLock<UdpSocket> = OnceLock::new();

#[derive(Serialize)]
struct UdpCommandPayload<'a> {
    command: &'a str,
    value: Option<i64>,
}

pub fn send_command(cmd: &Command, send_port: u16) -> AppResult<()> {
    let payload = match cmd {
        Command::Toggle => UdpCommandPayload {
            command: "toggle",
            value: None,
        },
        Command::Next => UdpCommandPayload {
            command: "next",
            value: None,
        },
        Command::Previous => UdpCommandPayload {
            command: "previous",
            value: None,
        },
        Command::Like => UdpCommandPayload {
            command: "like",
            value: None,
        },
        Command::Seek { position_ms } => UdpCommandPayload {
            command: "seek",
            value: Some(*position_ms),
        },
        Command::Volume { level } => UdpCommandPayload {
            command: "volume",
            value: Some(*level as i64),
        },
    };

    let serialized = serde_json::to_vec(&payload)?;

    // Reuse socket via OnceLock
    let socket = OUTBOUND_SOCKET.get_or_init(|| {
        UdpSocket::bind("127.0.0.1:0").expect("Failed to bind outbound UDP socket")
    });
    let target = format!("127.0.0.1:{}", send_port);
    socket.send_to(&serialized, &target)?;

    log::info!("Sent UDP command '{}' to {}", payload.command, target);

    Ok(())
}
