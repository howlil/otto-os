const INITIAL_STATE: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const ROUND_CONSTANTS: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

#[derive(Debug, Clone)]
pub struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    total_bytes: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: INITIAL_STATE,
            buffer: [0; 64],
            buffer_len: 0,
            total_bytes: 0,
        }
    }

    pub fn update(&mut self, mut input: &[u8]) {
        self.total_bytes = self.total_bytes.wrapping_add(input.len() as u64);
        if self.buffer_len > 0 {
            let take = (64 - self.buffer_len).min(input.len());
            self.buffer[self.buffer_len..self.buffer_len + take].copy_from_slice(&input[..take]);
            self.buffer_len += take;
            input = &input[take..];
            if self.buffer_len == 64 {
                let block = self.buffer;
                compress(&mut self.state, &block);
                self.buffer_len = 0;
            }
        }
        while input.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&input[..64]);
            compress(&mut self.state, &block);
            input = &input[64..];
        }
        if !input.is_empty() {
            self.buffer[..input.len()].copy_from_slice(input);
            self.buffer_len = input.len();
        }
    }

    #[must_use]
    pub fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total_bytes.wrapping_mul(8);
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;
        if self.buffer_len > 56 {
            self.buffer[self.buffer_len..64].fill(0);
            let block = self.buffer;
            compress(&mut self.state, &block);
            self.buffer.fill(0);
        } else {
            self.buffer[self.buffer_len..56].fill(0);
        }
        self.buffer[56..64].copy_from_slice(&bit_len.to_be_bytes());
        let block = self.buffer;
        compress(&mut self.state, &block);
        let mut digest = [0u8; 32];
        for (index, word) in self.state.iter().enumerate() {
            digest[index * 4..(index + 1) * 4].copy_from_slice(&word.to_be_bytes());
        }
        digest
    }
}

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut schedule = [0u32; 64];
    for (index, chunk) in block.chunks_exact(4).enumerate() {
        schedule[index] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    for index in 16..64 {
        let w15 = schedule[index - 15];
        let w2 = schedule[index - 2];
        let s0 = w15.rotate_right(7) ^ w15.rotate_right(18) ^ (w15 >> 3);
        let s1 = w2.rotate_right(17) ^ w2.rotate_right(19) ^ (w2 >> 10);
        schedule[index] = schedule[index - 16]
            .wrapping_add(s0)
            .wrapping_add(schedule[index - 7])
            .wrapping_add(s1);
    }
    let [
        mut w0,
        mut w1,
        mut w2,
        mut w3,
        mut w4,
        mut w5,
        mut w6,
        mut w7,
    ] = *state;
    for index in 0..64 {
        let s1 = w4.rotate_right(6) ^ w4.rotate_right(11) ^ w4.rotate_right(25);
        let ch = (w4 & w5) ^ ((!w4) & w6);
        let temp1 = w7
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(ROUND_CONSTANTS[index])
            .wrapping_add(schedule[index]);
        let s0 = w0.rotate_right(2) ^ w0.rotate_right(13) ^ w0.rotate_right(22);
        let maj = (w0 & w1) ^ (w0 & w2) ^ (w1 & w2);
        let temp2 = s0.wrapping_add(maj);
        w7 = w6;
        w6 = w5;
        w5 = w4;
        w4 = w3.wrapping_add(temp1);
        w3 = w2;
        w2 = w1;
        w1 = w0;
        w0 = temp1.wrapping_add(temp2);
    }
    state[0] = state[0].wrapping_add(w0);
    state[1] = state[1].wrapping_add(w1);
    state[2] = state[2].wrapping_add(w2);
    state[3] = state[3].wrapping_add(w3);
    state[4] = state[4].wrapping_add(w4);
    state[5] = state[5].wrapping_add(w5);
    state[6] = state[6].wrapping_add(w6);
    state[7] = state[7].wrapping_add(w7);
}

#[must_use]
pub fn sha256_digest(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize()
}

#[must_use]
pub fn parse_sha256_hex(hex: &str) -> Option<[u8; 32]> {
    let bytes = hex.trim().as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (index, pair) in bytes.chunks_exact(2).enumerate() {
        let hi = hex_nibble(pair[0])?;
        let lo = hex_nibble(pair[1])?;
        out[index] = (hi << 4) | lo;
    }
    Some(out)
}

#[must_use]
pub fn parse_sha256_for_asset(sums_text: &str, asset_name: &str) -> Option<[u8; 32]> {
    if asset_name.is_empty() {
        return None;
    }
    for line in sums_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let split_at = trimmed
            .bytes()
            .position(|byte| byte.is_ascii_whitespace())?;
        let hash_part = &trimmed[..split_at];
        let rest = trimmed[split_at..].trim_start();
        let candidate = rest.strip_prefix('*').unwrap_or(rest).trim();
        if candidate == asset_name {
            return parse_sha256_hex(hash_part);
        }
    }
    None
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Sha256, parse_sha256_for_asset, parse_sha256_hex, sha256_digest};

    #[test]
    fn matches_nist_fips_180_4_test_vectors() {
        assert_eq!(
            sha256_digest(b""),
            parse_sha256_hex("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                .unwrap()
        );
        assert_eq!(
            sha256_digest(b"abc"),
            parse_sha256_hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
                .unwrap()
        );
        assert_eq!(
            sha256_digest(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            parse_sha256_hex("248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1")
                .unwrap()
        );
        let million = vec![b'a'; 1_000_000];
        let mut streaming = Sha256::new();
        for chunk in million.chunks(777) {
            streaming.update(chunk);
        }
        assert_eq!(
            streaming.finalize(),
            parse_sha256_hex("cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0")
                .unwrap()
        );
    }

    #[test]
    fn parses_sha256sums_manifest_for_exact_asset_name() {
        let sums = concat!(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  otto-os-1.2.1-windows-x64.zip\n",
            "248D6A61D20638B8E5C026930C3E6039A33CE45964FF2167F6ECEDD419DB06C1 *otto-os-setup-1.2.1-x64.exe\n",
        );
        assert_eq!(
            parse_sha256_for_asset(sums, "otto-os-setup-1.2.1-x64.exe"),
            parse_sha256_hex("248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1")
        );
        assert_eq!(
            parse_sha256_for_asset(sums, "otto-os-setup-1.2.0-x64.exe"),
            None
        );
        assert_eq!(
            parse_sha256_for_asset(
                "not-a-hash  otto-os-setup-1.2.1-x64.exe",
                "otto-os-setup-1.2.1-x64.exe"
            ),
            None
        );
    }
}
