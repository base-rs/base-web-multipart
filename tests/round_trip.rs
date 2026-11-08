// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_multipart::{
    Boundary, MultipartEvent, MultipartParser, MultipartWriteError, MultipartWriter,
};

/// A small deterministic generator, so that failures reproduce.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }

    /// Bytes biased toward those that make up delimiters, so that near misses are common.
    fn content(&mut self, boundary: &[u8]) -> Vec<u8> {
        let length = self.below(48);
        let mut content = Vec::with_capacity(length);
        while content.len() < length {
            match self.below(6) {
                0 => content.extend_from_slice(b"\r\n"),
                1 => content.extend_from_slice(b"--"),
                2 => content.extend_from_slice(&boundary[..=self.below(boundary.len())]),
                3 => content.push(b'\r'),
                _ => content.push(self.next() as u8),
            }
        }
        content
    }
}

type Parts = Vec<(Vec<(String, Vec<u8>)>, Vec<u8>)>;

/// Parse a body fed in random chunk sizes.
fn parse(boundary: &Boundary, body: &[u8], random: &mut Random) -> Parts {
    let mut parser = MultipartParser::new(boundary);
    let (mut buffer, mut parts, mut rest) = (Vec::new(), Parts::new(), body);
    loop {
        let parsed = parser.parse(&buffer).unwrap();
        let consumed = parsed.consumed;
        match parsed.event {
            MultipartEvent::Part(head) => parts.push((
                head.fields()
                    .map(|(name, value)| (name.to_owned(), value.to_vec()))
                    .collect(),
                Vec::new(),
            )),
            MultipartEvent::Content(content) => {
                parts.last_mut().unwrap().1.extend_from_slice(content)
            }
            MultipartEvent::PartEnd => {}
            MultipartEvent::End => return parts,
            MultipartEvent::Incomplete => {
                assert!(
                    !rest.is_empty(),
                    "a complete body ends before its input does"
                );
                let (chunk, remaining) = rest.split_at((random.below(8) + 1).min(rest.len()));
                buffer.extend_from_slice(chunk);
                rest = remaining;
            }
        }
        buffer.drain(..consumed);
    }
}

#[test]
fn what_is_written_is_what_is_parsed() {
    let mut random = Random(0x9e37_79b9_7f4a_7c15);
    let boundary = Boundary::new("a-b").unwrap();
    let mut refused = 0;
    for _ in 0..2000 {
        let mut writer = MultipartWriter::new(boundary.clone());
        let (mut body, mut expected) = (Vec::new(), Parts::new());
        for index in 0..random.below(4) {
            let fields = vec![("X-Index".to_owned(), index.to_string().into_bytes())];
            writer
                .start_part(fields.iter().map(|(name, value)| (name, value)), &mut body)
                .unwrap();
            let mut content = Vec::new();
            for _ in 0..random.below(4) {
                let chunk = random.content(b"a-b");
                match writer.write_content(&chunk, &mut body) {
                    Ok(()) => content.extend_from_slice(&chunk),
                    Err(error) => {
                        assert_eq!(error, MultipartWriteError::Delimiter);
                        refused += 1;
                    }
                }
            }
            expected.push((fields, content));
        }
        writer.finish(&mut body);
        assert_eq!(
            parse(&boundary, &body, &mut random),
            expected,
            "{:?}",
            String::from_utf8_lossy(&body)
        );
    }
    assert!(
        refused > 0,
        "the generator produces delimiters, which the writer refuses"
    );
}
