// DJI Tello EDU Control Program

use std::{net::UdpSocket, io::stdin};
use std::sync::mpsc;

mod state;
mod video;

#[derive(PartialEq, Eq)]
pub enum ThreadMsg {
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

    // Channels for communicating with the state and video thread
    let (vtx, vrx) = mpsc::channel();
    let (stx, srx) = mpsc::channel();

    // Thread for receiving and processing video stream from the Tello
    let video_thread = video::spawn_video_thread(vrx);

    // Thread for getting state data from the Tello
    let state_thread = state::spawn_state_thread(srx);

    loop {
        let mut send_buf = String::new();

        stdin().read_line(&mut send_buf).unwrap();

        if send_buf.as_str().trim() == "q" {
            vtx.send(ThreadMsg::ShutdownThread);
            stx.send(ThreadMsg::ShutdownThread);
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
    state_thread.join();

}
