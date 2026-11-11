use serde_json::{json, Value};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

const CLIENT_ID: &str = "1005469189907173486";
const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;

pub struct DiscordRpc {
    pipe: Option<File>,
    pid: u32,
    nonce: u64,
}

impl DiscordRpc {
    pub fn new(pid: u32) -> Self {
        let mut rpc = Self {
            pipe: None,
            pid,
            nonce: 1,
        };
        rpc.connect();
        rpc
    }

    pub fn is_connected(&self) -> bool {
        self.pipe.is_some()
    }

    fn connect(&mut self) {
        for index in 0..10 {
            let path = format!(r"\\.\pipe\discord-ipc-{index}");
            let Ok(file) = OpenOptions::new().read(true).write(true).open(&path) else {
                continue;
            };

            self.pipe = Some(file);
            let handshake = json!({
                "v": 1,
                "client_id": CLIENT_ID,
            })
            .to_string();

            match self.send(OP_HANDSHAKE, &handshake) {
                Ok(()) => {
                    crate::logger::log("DiscordRPC", format!("Connected to {path}"));
                    return;
                }
                Err(error) => {
                    crate::logger::log("DiscordRPC", format!("Handshake failed: {error}"));
                    self.pipe = None;
                }
            }
        }

        crate::logger::log("DiscordRPC", "Discord is not running");
    }

    fn send(&mut self, opcode: u32, payload: &str) -> io::Result<()> {
        let Some(pipe) = self.pipe.as_mut() else {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "not connected"));
        };

        let length = payload.len() as u32;
        let mut frame = Vec::with_capacity(8 + payload.len());
        frame.extend_from_slice(&opcode.to_le_bytes());
        frame.extend_from_slice(&length.to_le_bytes());
        frame.extend_from_slice(payload.as_bytes());
        pipe.write_all(&frame)?;
        pipe.flush()
    }

    fn next_nonce(&mut self) -> String {
        self.nonce += 1;
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        format!("{millis}-{}", self.nonce)
    }

    pub fn set_activity(&mut self, activity: Value) -> bool {
        if self.pipe.is_none() {
            self.connect();
        }

        let nonce = self.next_nonce();
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": self.pid,
                "activity": activity,
            },
            "nonce": nonce,
        })
        .to_string();

        match self.send(OP_FRAME, &payload) {
            Ok(()) => true,
            Err(error) => {
                crate::logger::log("DiscordRPC", format!("Send failed: {error}"));
                self.pipe = None;
                self.connect();
                let nonce = self.next_nonce();
                let payload = json!({
                    "cmd": "SET_ACTIVITY",
                    "args": {
                        "pid": self.pid,
                        "activity": activity,
                    },
                    "nonce": nonce,
                })
                .to_string();
                self.send(OP_FRAME, &payload).is_ok()
            }
        }
    }

    pub fn clear_activity(&mut self) {
        if self.pipe.is_none() {
            return;
        }
        let nonce = self.next_nonce();
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": self.pid,
            },
            "nonce": nonce,
        })
        .to_string();
        let _ = self.send(OP_FRAME, &payload);
    }
}

pub fn unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}
