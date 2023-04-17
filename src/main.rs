use std::{net::UdpSocket, time::Duration, io::stdin, path::Path};
use openh264::decoder::Decoder;
use image;

fn check_for_valid_packet(data_stream: &[u8]) -> Option<(usize, usize)> {
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


fn h264_decode(frame_data: &[u8], decoder: &mut Decoder) -> Result<Vec<u8>, ()> {
    let mut buffer = vec![0; 2764800];
    let mut img_rendered = 0;
    let yuv = match decoder.decode(frame_data) {
        Ok(o) => {
            match o {
                Some(y) => {
                    img_rendered += 1;
                    y
                },
                None => {
                    return Err(());
                    //continue;
                },
            }
        },
        Err(_) => {
            return Err(());
            //continue;
        },
    };

    let dims = yuv.dimension_rgb();

    yuv.write_rgba8(&mut buffer);

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
        Ok(buffer)
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

        let mut decoder = Decoder::new().unwrap();
        let mut now = std::time::Instant::now();
        let mut a = 0;

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

            match check_for_valid_packet(&frame_packet) {
                Some(frame_borders) => {
                    match h264_decode(&frame_packet[(frame_borders.0)..(frame_borders.1)], &mut decoder) {
                        Ok(_) => {
                            frame_packet = frame_packet[(frame_borders.1)..(frame_packet.len())].to_vec();
                            
                            a += 1;
                            if now.elapsed().as_secs_f64() >= 1.0 {
                                now = std::time::Instant::now();
                                println!("Got: {}", a);
                                a = 0;
                            }
                        },
                        Err(_) => {
                            frame_packet = frame_packet[(frame_borders.1)..(frame_packet.len())].to_vec();
                        }
                    };
                },
                None => continue,
            }

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
