// H.264 decoding code

pub use openh264::decoder::Decoder;

/// Find two H.264 packet beginnings, and return them if they exist.
pub fn check_for_valid_packet(data_stream: &[u8]) -> Option<(usize, usize)> {
    let mut zero_seq_len = 0;
    let mut first = None;

    for i in 0..data_stream.len() {
        match data_stream[i] {
            0 => {
                zero_seq_len += 1;
            },
            1 => {
                if zero_seq_len >= 2 {
                    if first != None {
                        return Some((first.unwrap(), i - 2));
                    } else {
                        first = Some(i - 2);
                    }
                } 
            }
            _ => {
                zero_seq_len = 0;
            }
        }
    }

    None
}

pub fn h264_decode(frame_data: &[u8], decoder: &mut Decoder) -> Result<(Vec<u8>, (usize, usize)), ()> {
    // Tello EDU, by default, has a resolution of 960 * 720, multiply that with 4, for the 4
    // channels in RGBA, and we get 2764800. Probably shouldn't be hardcoded.
    let mut buffer = vec![0; 2764800];
    let yuv = match decoder.decode(frame_data) {
        Ok(o) => {
            match o {
                Some(y) => {
                    y
                },
                None => {
                    return Err(());
                },
            }
        },
        Err(_) => {
            return Err(());
        },
    };

    let dims = yuv.dimension_rgb();
    yuv.write_rgba8(&mut buffer);

    Ok((buffer, dims))
}
