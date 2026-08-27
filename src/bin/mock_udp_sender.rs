use lyric_for_musicfox::lyric::udp::{UdpLineInfo, UdpLyricPayload, UdpSongInfo};
use std::net::UdpSocket;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut target = "127.0.0.1:16501".to_string();
    let mut interval_ms = 100;
    let mut count = 0;
    let mut payload_type = "lyric_update".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--target" | "-t" => {
                if i + 1 < args.len() {
                    target = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("Missing value for --target");
                    std::process::exit(1);
                }
            }
            "--interval-ms" | "-i" => {
                if i + 1 < args.len() {
                    interval_ms = args[i + 1].parse()?;
                    i += 2;
                } else {
                    eprintln!("Missing value for --interval-ms");
                    std::process::exit(1);
                }
            }
            "--count" | "-c" => {
                if i + 1 < args.len() {
                    count = args[i + 1].parse()?;
                    i += 2;
                } else {
                    eprintln!("Missing value for --count");
                    std::process::exit(1);
                }
            }
            "--type" => {
                if i + 1 < args.len() {
                    payload_type = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("Missing value for --type");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                eprintln!("Usage: mock_udp_sender [--target <ip:port>] [--interval-ms <ms>] [--count <num>] [--type <type>]");
                std::process::exit(1);
            }
        }
    }

    println!("Mock UDP sender starting...");
    println!("Target: {}", target);
    println!("Interval: {} ms", interval_ms);
    println!(
        "Count limit: {}",
        if count == 0 {
            "unlimited".to_string()
        } else {
            count.to_string()
        }
    );
    println!("Type: {}", payload_type);

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let mut time_ms = 0;
    let mut sent = 0;

    let lyrics = [
        "这是一首简单的歌，没有什么特别的意义 ～～",
        "这是一测试 UDP 歌词滚动的极长文本 1234567890 极长文本且字数特别多以至于超过了八百像素宽度从而必然触发横向滚动",
        "切换到中等长度的第三行歌词测试",
        "短暂的播放暂停测试（透明度下降）",
        "测试完毕，准备进入循环循环...",
    ];

    loop {
        let lyric_idx = ((time_ms / 3000) % lyrics.len() as i64) as usize;
        let is_paused = lyric_idx == 3; // Let idx 3 simulate pause
        let current_text = lyrics[lyric_idx];

        let payload = UdpLyricPayload {
            type_: payload_type.clone(),
            playing: !is_paused,
            time_ms,
            volume: 80,
            mode: "list-loop".to_string(),
            song: UdpSongInfo {
                id: 112233,
                name: "测试歌曲".to_string(),
                artist: "测试艺术家".to_string(),
                album: "测试专辑".to_string(),
                pic_url: "http://example.com/pic.jpg".to_string(),
                duration: 180000,
                is_favorite: true,
            },
            current_line: UdpLineInfo {
                text: current_text.to_string(),
                words: vec![],
            },
            next_line: UdpLineInfo {
                text: lyrics[(lyric_idx + 1) % lyrics.len()].to_string(),
                words: vec![],
            },
        };

        let bytes = serde_json::to_vec(&payload)?;
        socket.send_to(&bytes, &target)?;

        sent += 1;
        if count > 0 && sent >= count {
            break;
        }

        time_ms += interval_ms as i64;
        std::thread::sleep(Duration::from_millis(interval_ms));
    }

    println!("Sent {} packets. Done.", sent);
    Ok(())
}
