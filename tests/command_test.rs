use lyric_for_musicfox::command::{send_command, Command};
use std::net::UdpSocket;
use std::time::Duration;

#[test]
fn test_command_serialization() {
    // We can serialise the inner representation to test that the schema is correct.
    // However, since UdpCommandPayload is private in udp_send, we can verify via sending to a socket.
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    server
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();

    // Test Toggle
    send_command(&Command::Toggle, port).unwrap();
    let mut buf = [0u8; 1024];
    let (len, _) = server.recv_from(&mut buf).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&buf[..len]).unwrap();
    assert_eq!(json["command"], "toggle");
    assert!(json["value"].is_null());

    // Test Seek
    send_command(&Command::Seek { position_ms: 12345 }, port).unwrap();
    let (len, _) = server.recv_from(&mut buf).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&buf[..len]).unwrap();
    assert_eq!(json["command"], "seek");
    assert_eq!(json["value"], 12345);

    // Test Volume
    send_command(&Command::Volume { level: 80 }, port).unwrap();
    let (len, _) = server.recv_from(&mut buf).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&buf[..len]).unwrap();
    assert_eq!(json["command"], "volume");
    assert_eq!(json["value"], 80);

    // Test Like
    send_command(&Command::Like, port).unwrap();
    let (len, _) = server.recv_from(&mut buf).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&buf[..len]).unwrap();
    assert_eq!(json["command"], "like");
    assert!(json["value"].is_null());

    // Test Next
    send_command(&Command::Next, port).unwrap();
    let (len, _) = server.recv_from(&mut buf).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&buf[..len]).unwrap();
    assert_eq!(json["command"], "next");
    assert!(json["value"].is_null());

    // Test Previous
    send_command(&Command::Previous, port).unwrap();
    let (len, _) = server.recv_from(&mut buf).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&buf[..len]).unwrap();
    assert_eq!(json["command"], "previous");
    assert!(json["value"].is_null());
}
