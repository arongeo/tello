// DJI Tello EDU Control Program

use std::{net::UdpSocket, io::stdin};
use openh264::decoder::Decoder;
use opencv as cv;
use cv::{
    highgui,
    prelude::Mat,
    imgproc::{cvt_color, COLOR_RGBA2BGRA, COLOR_BGRA2GRAY},
    objdetect::CascadeClassifier,
    types::VectorOfRect,
};
use std::sync::mpsc;

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


fn h264_decode_to_bgra(frame_data: &[u8], decoder: &mut Decoder) -> Result<(Mat, (usize, usize)), ()> {
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

    Ok((bgra_buffer, dims))
}

fn to_grayscale(matrix: &Mat, dims: (usize, usize)) -> Result<Mat, ()> {
    let mut gray_buf = unsafe { Mat::new_rows_cols(dims.1 as i32, dims.0 as i32, 0).unwrap() };
    match cvt_color(matrix, &mut gray_buf, COLOR_BGRA2GRAY, 0) {
        Ok(_) => {},
        Err(e) => {
            println!("Error failed to create gray matrix: {}", e);
            return Err(());
        },
    };
    return Ok(gray_buf);
}

#[derive(PartialEq, Eq)]
enum ThreadMsg {
    None,
    ShutdownThread,
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

    let (tx, rx) = mpsc::channel();

    let thread_thing = std::thread::spawn(move || {
        // receive video stream from tello on port 11111
        let video_socket = match UdpSocket::bind("0.0.0.0:11111") {
            Ok(s) => s,
            Err(e) => panic!("ERROR with creating socket: {}", e),
        };
    
        let _win = highgui::named_window("tello", highgui::WINDOW_AUTOSIZE);
        let _gray_win = highgui::named_window("graytello", highgui::WINDOW_AUTOSIZE);

        let mut frame_packet = vec![];

        let mut decoder = Decoder::new().unwrap();

        loop {
            let mut video_buffer = [0; 2048];
            let msg_len = match video_socket.recv(&mut video_buffer) {
                Ok(ml) => ml,
                Err(_) => continue,
            };

            frame_packet.extend_from_slice(&video_buffer[..msg_len]);

            match check_for_valid_packet(&frame_packet) {
                Some(frame_borders) => {
                    match h264_decode_to_bgra(&frame_packet[(frame_borders.0)..(frame_borders.1)], &mut decoder) {
                        Ok(result) => {
                            frame_packet = frame_packet[(frame_borders.1)..(frame_packet.len())].to_vec();
                            
                            match highgui::imshow("tello", &result.0) {
                                Ok(_) => {},
                                Err(e) => println!("Error: {:?}", e),
                            };

                            let gray_buf = match to_grayscale(&result.0, result.1) {
                                Ok(m) => m,
                                Err(_) => continue,
                            };

                            match highgui::imshow("graytello", &gray_buf) {
                                Ok(_) => {},
                                Err(e) => println!("Error: {:?}", e),
                            };

                            highgui::poll_key();

                            match rx.try_recv() {
                                Ok(v) => {
                                    if v == ThreadMsg::ShutdownThread {
                                        break;
                                    }
                                },
                                Err(_) => {},
                            };
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
            tx.send(ThreadMsg::ShutdownThread);
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
