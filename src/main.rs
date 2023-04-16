use std::{net::UdpSocket, time::Duration, io::stdin, path::Path};
use openh264::decoder::Decoder;
use image;

const NAL_MIN_0_COUNT: usize = 2;

/// Given a stream, finds the index of the nth NAL start.
fn nth_nal_index(stream: &[u8], nth: usize) -> Option<usize> {
    let mut count_0 = 0;
    let mut n = 0;

    for (i, byte) in stream.iter().enumerate() {
        match byte {
            0 => count_0 += 1,
            1 if count_0 >= NAL_MIN_0_COUNT => {
                if n == nth {
                    return Some(i - NAL_MIN_0_COUNT);
                } else {
                    count_0 = 0;
                    n += 1;
                }
            }
            _ => count_0 = 0,
        }
    }

    None
}

pub fn nal_units(mut stream: &[u8]) -> impl Iterator<Item = &[u8]> {
    std::iter::from_fn(move || {
        let first = nth_nal_index(stream, 0);
        let next = nth_nal_index(stream, 1);

        match (first, next) {
            (Some(f), Some(n)) => {
                let rval = &stream[f..n];
                stream = &stream[n..];
                Some(rval)
            }
            (Some(f), None) => {
                let rval = &stream[f..];
                stream = &stream[f + NAL_MIN_0_COUNT..];
                Some(rval)
            }
            _ => None,
        }
    })
}

fn h264_decode(frame_data: &[u8]) -> Result<i32, ()> {
    let mut decoder = Decoder::new().unwrap();

    let mut buffer = vec![0; 2764800];
    let mut img_rendered = 0;
    for data in nal_units(frame_data) {
        let yuv = match decoder.decode(data) {
            Ok(o) => {
                match o {
                    Some(y) => {
                        println!("Decoded at {}", frame_data.len());
                        img_rendered += 1;
                        y
                    },
                    None => {
                        //return Err(());
                        continue;
                    },
                }
            },
            Err(_) => {
                //return Err(());
                continue;
            },
        };


        let dims = yuv.dimension_rgb();

        yuv.write_rgba8(&mut buffer);


    }
    /*
    print!("COM: [");
    for i in 0..data.len() {
        print!("{}, ", data[i]);
    }
    print!("]\n");
    */

    /*
    print!("ME: [");
    for i in 0..according_to_me.len() {
        print!("{}, ", according_to_me[i]);
    }
    print!("]\n");
    */
        
    if 0 < img_rendered {
        Ok(img_rendered as i32)
    } else {
        Err(())
    }
}

fn main() {
    let sock = match UdpSocket::bind("0.0.0.0:62000") {
        Ok(s) => s,
        Err(e) => panic!("ERROR with creating socket: {}", e),
    };
    
    sock.connect("192.168.10.1:8889");

    sock.send(&("command".as_bytes()));

    let mut buffer = [0; 2048];
    match sock.recv(&mut buffer) {
        Ok(_) => {},
        Err(_) => panic!(""),
    };

    let thread_thing = std::thread::spawn(|| {
        // receive video stream from tello on port 11111
        let mut video_socket = match UdpSocket::bind("0.0.0.0:11111") {
            Ok(s) => s,
            Err(e) => panic!("ERROR with creating socket: {}", e),
        };
    
        let mut frame_packet = vec![];
        let mut ptr = 0;
        let mut a = 0;
        let mut lost = 0;

        let mut now = std::time::Instant::now();

        loop {
            let mut video_buffer = [0; 1460];
            let msg_len = match video_socket.recv(&mut video_buffer) {
                Ok(ml) => ml,
                Err(_) => continue,
            };

            /*
            frame_packet[ptr..(ptr+msg_len)].clone_from_slice(&video_buffer[..msg_len]); //video_buffer[i - ptr];
            ptr += msg_len;
            */

            /*
            for i in 0..(msg_len) {
                frame_packet[i + ptr] = video_buffer[i];
            }
            ptr += msg_len;
            */

            frame_packet.extend_from_slice(&video_buffer[..msg_len]);
            ptr += msg_len;

            /*
            let mut first = nth_nal_index(&frame_packet, 0);
            let mut second = nth_nal_index(&frame_packet, 1);
            if (first != None) & (second != None) {
                let first_i = first.unwrap();
                let second_i = second.unwrap();

                match h264_decode(&frame_packet[first_i..second_i]) {
                    Ok(_) => {
                        let mut frame_packet_replace = [0; 20000];
                        frame_packet_replace[0..(ptr-second_i)].clone_from_slice(&frame_packet[second_i..ptr]);
                        frame_packet = frame_packet_replace;
                        ptr = ptr - second_i;

                        a += 1;

                        if now.elapsed().as_secs_f64() >= 1.0 {
                            now = std::time::Instant::now();
                            println!("Got: {}", a);
                            a = 0;
                        }
                    },
                    Err(_) => {
                        let mut frame_packet_replace = [0; 20000];
                        frame_packet_replace[0..(ptr-second_i)].clone_from_slice(&frame_packet[second_i..ptr]);
                        frame_packet = frame_packet_replace;
                        ptr = ptr - second_i;

                    }
                };
            }
            */


            match h264_decode(&frame_packet) {
                Ok(nof) => {
                    println!("Frames decoded: {}", nof);
                },
                Err(_) => continue,
            };
        }
    });

    loop {
        let mut send_buf = String::new();

        stdin().read_line(&mut send_buf).unwrap();

        if send_buf.as_str().trim() == "q" {
            break;
        }

        buffer = [0; 2048];

        sock.send(&(send_buf.as_str().trim().as_bytes()));

        match sock.recv(&mut buffer) {
            Ok(_) => {},
            Err(_) => panic!(""),
        };

        println!("{}", std::str::from_utf8(&buffer).unwrap());

    }

    thread_thing.join();

}
