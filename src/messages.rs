#[non_exhaustive]
pub struct MessageID;

// TODO: this can probably be represented better than just returning a Vec<u8>
pub trait Sendable {
    fn to_bytes(self) -> Vec<u8>;
}

impl MessageID {
    pub const HELLO_MESSAGE: u8 = 0x00;
    pub const VERSION_MESSAGE: u8 = 0x01;
}

#[derive(Debug)]
pub struct HelloMessage {
    id: u8,
    version_length: u8,
    versions: Vec<u8>,
}

impl HelloMessage {
    pub fn new(versions: &[u8]) -> Self {
        HelloMessage {
            id: MessageID::HELLO_MESSAGE,
            version_length: versions.len() as u8,
            versions: Vec::from(versions)
        }
    }
}

impl Default for HelloMessage {
    fn default() -> Self {
        Self {
            id: MessageID::HELLO_MESSAGE,
            version_length: 1,
            versions: vec![0x1],
        }
    }
}

impl Sendable for HelloMessage {
    // Layout of hello_message:
    // 1 byte:  msg_type
    // 1 byte:  length of versions
    // n bytes: server supported versions 
    fn to_bytes(mut self) -> Vec<u8> {
        let mut v = vec![self.id, self.version_length];
        v.append(&mut self.versions);

        v
    }
}

pub struct VersionMessage {
    id: u8,
    version: u8,
}

impl VersionMessage {
    pub fn new(version: u8) -> Self {
        VersionMessage { id: MessageID::VERSION_MESSAGE, version }
    }
}

impl Sendable for VersionMessage {
    fn to_bytes(self) -> Vec<u8> {
        vec![self.id, self.version]
    }
}

mod test {
    use super::*;
    #[test]
    fn test_hello_message() {
        let expected: Vec<u8> = vec![0x00, 0x01, 0x01];
        let v = HelloMessage::default().to_bytes();
        assert_eq!(expected, v);

        let expected: Vec<u8> = vec![0x00, 0x03, 0x01, 0x02, 0x03];
        let v = HelloMessage::new(&[0x01, 0x02, 0x03]).to_bytes();
        assert_eq!(expected, v);
    }   
}