#[derive(Debug, PartialEq, Eq, Clone)]
pub enum IpcMessage {
    ReloadConfig,
    GetPosition,
    PositionResponse(Option<(i32, i32)>),
    ActivateSettings(u32), // PID
    Success,
}

impl IpcMessage {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::ReloadConfig => b"RELOAD\n".to_vec(),
            Self::GetPosition => b"GET_POS\n".to_vec(),
            Self::PositionResponse(Some((x, y))) => format!("POS {},{}\n", x, y).into_bytes(),
            Self::PositionResponse(None) => b"POS EMPTY\n".to_vec(),
            Self::ActivateSettings(pid) => format!("ACTIVATE {}\n", pid).into_bytes(),
            Self::Success => b"OK\n".to_vec(),
        }
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let s = std::str::from_utf8(bytes)
            .map_err(|e| e.to_string())?
            .trim();
        if s == "RELOAD" {
            return Ok(Self::ReloadConfig);
        }
        if s == "GET_POS" {
            return Ok(Self::GetPosition);
        }
        if s == "POS EMPTY" {
            return Ok(Self::PositionResponse(None));
        }
        if s.starts_with("POS ") {
            let coords = &s[4..];
            let parts: Vec<&str> = coords.split(',').collect();
            if parts.len() == 2 {
                let x = parts[0].trim().parse::<i32>().map_err(|e| e.to_string())?;
                let y = parts[1].trim().parse::<i32>().map_err(|e| e.to_string())?;
                return Ok(Self::PositionResponse(Some((x, y))));
            }
        }
        if s.starts_with("ACTIVATE ") {
            let pid = s[9..].trim().parse::<u32>().map_err(|e| e.to_string())?;
            return Ok(Self::ActivateSettings(pid));
        }
        if s == "OK" {
            return Ok(Self::Success);
        }
        Err(format!("invalid protocol message: {}", s))
    }
}

pub fn get_pipe_path(base: &str, session_id: u32) -> String {
    format!("lyric-for-musicfox-{}-{}", base, session_id)
}

pub fn platform_pipe_path(base: &str, session_id: u32) -> String {
    #[cfg(windows)]
    {
        format!(r"\\.\pipe\{}", get_pipe_path(base, session_id))
    }
    #[cfg(not(windows))]
    {
        get_pipe_path(base, session_id)
    }
}

pub fn get_session_id() -> u32 {
    crate::platform::current().session_id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_reload() {
        let msg = IpcMessage::ReloadConfig;
        let bytes = msg.to_bytes();
        assert_eq!(bytes, b"RELOAD\n");
        let parsed = IpcMessage::parse(&bytes).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn parse_pos_empty() {
        let msg = IpcMessage::PositionResponse(None);
        let bytes = msg.to_bytes();
        assert_eq!(bytes, b"POS EMPTY\n");
        let parsed = IpcMessage::parse(&bytes).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn parse_pos_with_negatives() {
        let msg = IpcMessage::PositionResponse(Some((-100, 200)));
        let bytes = msg.to_bytes();
        assert_eq!(bytes, b"POS -100,200\n");
        let parsed = IpcMessage::parse(&bytes).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn parse_pos_rejects_three_tokens() {
        let res = IpcMessage::parse(b"POS 1,2,3");
        assert!(res.is_err());
    }

    #[test]
    fn parse_rejects_garbage() {
        let res = IpcMessage::parse(b"GARBAGE");
        assert!(res.is_err());
    }

    #[test]
    fn test_trailing_crlf_flexibility() {
        let parsed1 = IpcMessage::parse(b"RELOAD\r\n").unwrap();
        assert_eq!(parsed1, IpcMessage::ReloadConfig);

        let parsed2 = IpcMessage::parse(b"POS 123,-456\r\n").unwrap();
        assert_eq!(parsed2, IpcMessage::PositionResponse(Some((123, -456))));
    }

    #[test]
    fn get_pipe_path_format() {
        let path = get_pipe_path("reload", 5);
        assert_eq!(path, "lyric-for-musicfox-reload-5");
    }
}
