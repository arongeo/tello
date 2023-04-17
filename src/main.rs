use std::{net::UdpSocket, time::Duration, io::stdin, path::Path};
use ffmpeg::codec::Context;
use ffmpeg::codec::traits::Decoder;
use ffmpeg_next as ffmpeg;
use ffmpeg::util::frame::Video as VFrame;
use ffmpeg::software::scaling::{context::Context as ScalingContext, flag::Flags};
use ffmpeg::{Packet, Frame};
use image;

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
        let video_socket = match UdpSocket::bind("0.0.0.0:11111") {
            Ok(s) => s,
            Err(e) => panic!("ERROR with creating socket: {}", e),
        };

        ffmpeg::init().unwrap();

        let codec = ffmpeg::decoder::find(ffmpeg::codec::Id::H264).unwrap();

        let context = Context::new();
        let decoder = context.decoder();
        let opened = match decoder.open_as(codec) {
            Ok(opened) => opened,
            Err(e) => panic!("Failed to open decoder: {}", e),
        };

        let mut vdecoder = match opened.video() {
            Ok(vd) => vd,
            Err(e) => panic!("Couldn't get video decoder from decoder: {}", e),
        };

        let mut now = std::time::Instant::now();
        let mut frame_counter = 0;

        let mut frame_buffer = vec![];
        loop {
            let mut vbuffer = [0; 1460];
            
            let msg_len = match video_socket.recv(&mut vbuffer) {
                Ok(l) => l,
                Err(_) => continue,
            };

            frame_buffer.extend_from_slice(&vbuffer[..msg_len]);

            if msg_len < 15 {
                continue;
            }

            if (msg_len < 1460) {
                let packet = Packet::copy(&frame_buffer);

                let mut frame = unsafe { Frame::empty() };

                match vdecoder.send_packet(&packet) {
                    Ok(_) => {},
                    Err(e) => println!("Error with sending the packet: {:?}", e),
                }

                match vdecoder.receive_frame(&mut frame) {
                    Ok(_) => { 

                        let mut scaler = ScalingContext::get(
                            vdecoder.format(),
                            vdecoder.width(),
                            vdecoder.height(),
                            ffmpeg::format::Pixel::RGB24,
                            vdecoder.width(),
                            vdecoder.height(),
                            Flags::BILINEAR,
                        ).unwrap();

                        frame_counter += 1;
                        if now.elapsed().as_secs_f64() > 1.0 {
                            frame_counter = 0;
                            now = std::time::Instant::now();
                        }

                    },
                    Err(e) => {
                        std::thread::sleep_ms(10);
                    },
                };

                frame_buffer = vec![];
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
