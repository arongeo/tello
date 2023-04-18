use std::{net::UdpSocket, io::stdin};
use cv::highgui;
use openh264::decoder::Decoder;
use opencv as cv;
use cv::prelude::Mat;
use cv::imgproc::{cvt_color, COLOR_RGBA2BGRA};

use libc::c_void;

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


fn h264_decode(frame_data: &[u8], decoder: &mut Decoder) -> Result<Mat, ()> {
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

    let mut bgra_buffer = unsafe { Mat::new_rows_cols(dims.1 as i32, dims.0 as i32, cv::core::CV_8UC4).unwrap() };

    let frame = unsafe { Mat::new_rows_cols_with_data(
            dims.1 as i32,
            dims.0 as i32,
            cv::core::CV_8UC4,
            buffer.as_mut_ptr() as *mut c_void,
            0,
        ).unwrap() 
    };
    
    match cvt_color(&frame, &mut bgra_buffer, COLOR_RGBA2BGRA, 0) {
        Ok(_) => {},
        Err(e) => println!("Error image conversion failed: {:?}", e),
    };
    
    //println!("{}", bgra_buffer.at_pt_mut::<u8>(cv::core::Point_ { x: 0, y: 0 }).unwrap());

    Ok(bgra_buffer)
}

fn main() {
    let sock = match UdpSocket::bind("0.0.0.0:42069") {
        Ok(s) => s,
        Err(e) => panic!("ERROR with creating socket: {}", e),
    };
    
    match sock.connect("192.168.10.1:8889") {
        Ok(_) => {},
        Err(e) => panic!("Failed to connect: {}", e),
    };

    sock.send(&("command".as_bytes()));

    let mut buffer = [0; 2048];
    match sock.recv(&mut buffer) {
        Ok(_) => {},
        Err(_) => panic!(""),
    };

    let thread_thing = std::thread::spawn(|| {
        // receive video stream from tello on port 11111
        let video_socket = match UdpSocket::bind("0.0.0.0:11111") {
            Ok(s) => s,
            Err(e) => panic!("ERROR with creating socket: {}", e),
        };
    
        let _win = highgui::named_window("tello", highgui::WINDOW_AUTOSIZE);

        let mut frame_packet = vec![];

        let mut decoder = Decoder::new().unwrap();

        loop {
            let mut video_buffer = [0; 1460];
            let msg_len = match video_socket.recv(&mut video_buffer) {
                Ok(ml) => ml,
                Err(_) => continue,
            };

            frame_packet.extend_from_slice(&video_buffer[..msg_len]);

            match check_for_valid_packet(&frame_packet) {
                Some(frame_borders) => {
                    match h264_decode(&frame_packet[(frame_borders.0)..(frame_borders.1)], &mut decoder) {
                        Ok(frbuf) => {
                            frame_packet = frame_packet[(frame_borders.1)..(frame_packet.len())].to_vec();
                            
                            //frame = unsafe { Mat::new_rows_cols_with_data(dims.0 as i32, dims.1 as i32, cv::core::CV_8UC4, frbuf.as_mut_ptr() as *mut c_void, 1).unwrap() };
                
                            match highgui::imshow("tello", &frbuf) {
                                Ok(_) => {},
                                Err(e) => println!("Error: {:?}", e),
                            };

                            highgui::poll_key();

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
