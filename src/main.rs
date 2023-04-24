// DJI Tello EDU Control Program

use std::{net::UdpSocket, io::stdin};
use std::sync::mpsc;
use opencv as cv;
use cv::{
    highgui,
    objdetect::CascadeClassifier,
    types::VectorOfRect,
};

mod decoding;
mod conversions;

#[derive(PartialEq, Eq)]
enum ThreadMsg {
    ShutdownThread,
}

#[derive(PartialEq)]
struct TelloState {
    pub mid: i32,
    pub xyz: [i32; 3],
    pub mpry: [i32; 3],
    pub pitch: i32,
    pub roll: i32,
    pub yaw: i32,
    pub vg: [i32; 3],
    pub temp: [i32; 2],
    pub tof: i32,
    pub h: i32,
    pub bat: u32,
    pub baro: f64,
    pub time: i32,
    pub ag: [f64; 3],
}

impl TelloState {
    pub fn new() -> Self {
        Self {
            mid: 0,
            xyz: [0; 3],
            mpry: [0; 3],
            pitch: 0,
            roll: 0,
            yaw: 0,
            vg: [0; 3],
            temp: [0; 2],
            tof: 0,
            h: 0,
            bat: 0,
            baro: 0.0,
            time: 0,
            ag: [0.0; 3],
        }
    }
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

    let video_thread = std::thread::spawn(move || {
        // receive video stream from tello on port 11111
        let video_socket = match UdpSocket::bind("0.0.0.0:11111") {
            Ok(s) => s,
            Err(e) => panic!("ERROR with creating socket: {}", e),
        };
    
        let _win = highgui::named_window("tello", highgui::WINDOW_AUTOSIZE);
        let _gray_win = highgui::named_window("graytello", highgui::WINDOW_AUTOSIZE);

        let mut frame_packet = vec![];

        let mut decoder = decoding::Decoder::new().unwrap();

        loop {
            let mut video_buffer = [0; 2048];
            let msg_len = match video_socket.recv(&mut video_buffer) {
                Ok(ml) => ml,
                Err(_) => continue,
            };

            frame_packet.extend_from_slice(&video_buffer[..msg_len]);

            match decoding::check_for_valid_packet(&frame_packet) {
                Some(frame_borders) => {
                    match decoding::h264_decode(&frame_packet[(frame_borders.0)..(frame_borders.1)], &mut decoder) {
                        Ok(mut result) => {
                            frame_packet = frame_packet[(frame_borders.1)..(frame_packet.len())].to_vec();
                            
                            let bgra_frame = match conversions::vec_to_bgra_mat(&mut result.0, result.1) {
                                Ok(m) => m,
                                Err(()) => continue,
                            };

                            match highgui::imshow("tello", &bgra_frame) {
                                Ok(_) => {},
                                Err(e) => println!("Error: {:?}", e),
                            };

                            let gray_buf = match conversions::mat_to_grayscale(&bgra_frame, result.1) {
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

    let state_thread = std::thread::spawn(move || {
        let state_socket = match UdpSocket::bind("0.0.0.0:8890") {
            Ok(s) => s,
            Err(e) => panic!("ERROR with creating socket: {}", e),
        };

        loop {
            let mut buffer = [0; 2048];
            let msg_len = match state_socket.recv(&mut buffer) {
                Ok(l) => l,
                Err(_) => continue,
            };

            let state_str = match std::str::from_utf8(&buffer[..msg_len]) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let states = state_str.split(";").collect::<Vec<&str>>();

            let mut tellostate = TelloState::new();

            for (i, state) in states.iter().enumerate() {
                let mut num = state.chars().collect::<Vec<char>>();
                num.retain(|n| (n.is_digit(10)) | (*n == '.') | (*n == ',') | (*n == '-'));

                let str_num = String::from_iter(num);

                match i {
                    0 => tellostate.mid = str_num.parse().unwrap(),
                    (1..=3) => tellostate.xyz[i-1] = str_num.parse().unwrap(),
                    4 => {
                        for (j, val) in str_num.split(",").into_iter().enumerate() {
                            tellostate.mpry[j] = val.parse().unwrap();
                        }
                    },
                    5 => tellostate.pitch = str_num.parse().unwrap(),
                    6 => tellostate.roll = str_num.parse().unwrap(),
                    7 => tellostate.yaw = str_num.parse().unwrap(),
                    (8..=10) => tellostate.vg[i-8] = str_num.parse().unwrap(),
                    (11..=12) => tellostate.temp[i-11] = str_num.parse().unwrap(),
                    13 => tellostate.tof = str_num.parse().unwrap(),
                    14 => tellostate.h = str_num.parse().unwrap(),
                    15 => tellostate.bat = str_num.parse().unwrap(),
                    16 => tellostate.baro = str_num.parse().unwrap(),
                    17 => tellostate.time = str_num.parse().unwrap(),
                    (18..=20) => tellostate.ag[i-18] = str_num.parse().unwrap(),
                    _ => {},
                }
            }

            println!("{}%", tellostate.bat);
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

    video_thread.join();

}
