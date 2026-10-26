use crate::decode::BencodedValue;

pub fn encode(data: &BencodedValue) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();

    encode_data(data, &mut buffer);
    buffer
}

fn encode_data(data: &BencodedValue, buffer: &mut Vec<u8>) {
    match data {
        BencodedValue::Integer(i) => {
            buffer.push(b'i');
            buffer.extend_from_slice(i.to_string().as_bytes());
            buffer.push(b'e');
        }
        BencodedValue::String(s) => {
            buffer.extend_from_slice(s.len().to_string().as_bytes());
            buffer.push(b':');
            buffer.extend_from_slice(s);
        }
        BencodedValue::List(l) => {
            buffer.push(b'l');
            for item in l {
                encode_data(item, buffer);
            }
            buffer.push(b'e');
        }
        BencodedValue::Dictionary(d) => {
            buffer.push(b'd');

            let mut items: Vec<_> = d.iter().collect();

            // sorting lexicographically
            items.sort_by(|a, b| a.0.cmp(b.0));

            for (key, value) in items {
                // key
                buffer.extend_from_slice(key.len().to_string().as_bytes());

                buffer.push(b':');
                buffer.extend_from_slice(key);

                // value
                encode_data(value, buffer);
            }

            buffer.push(b'e');
        }
    }
}
