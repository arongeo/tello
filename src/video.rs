
use opencv as cv;
use cv::{
    highgui,
    objdetect::{CascadeClassifier, self},
    types::VectorOfRect, prelude::CascadeClassifierTrait,
    core::Size,
    imgproc::rectangle,
};

mod decoding;
mod conversions;

pub fn spawn_video_thread(vrx: std::sync::mpsc::Receiver<crate::ThreadMsg>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        // receive video stream from tello on port 11111
        let video_socket = match crate::UdpSocket::bind("0.0.0.0:11111") {
            Ok(s) => s,
            Err(e) => panic!("ERROR with creating socket: {}", e),
        };
    
        let _win = highgui::named_window("tello", highgui::WINDOW_AUTOSIZE);
        let _gray_win = highgui::named_window("graytello", highgui::WINDOW_AUTOSIZE);

        let mut frame_packet = vec![];

        let mut decoder = decoding::Decoder::new().unwrap();

        let mut face_detector = match CascadeClassifier::new("./cascades/haarcascade_frontalface_alt.xml") {
            Ok(cc) => cc,
            Err(e) => panic!("Failed to create face detector: {:?}", e),
        };

        let mut faces = VectorOfRect::new();
        let mut find_face_in = 3;

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
                            
                            let mut bgra_frame = match conversions::vec_to_bgra_mat(&mut result.0, result.1) {
                                Ok(m) => m,
                                Err(()) => continue,
                            };

                            let gray_buf = match conversions::mat_to_grayscale(&bgra_frame, result.1) {
                                Ok(m) => m,
                                Err(_) => continue,
                            };

                            match highgui::imshow("graytello", &gray_buf) {
                                Ok(_) => {},
                                Err(e) => println!("Error: {:?}", e),
                            };

                            find_face_in -= 1;

                            // We need to limit how much the classifier is ran, for performance
                            // reasons.
                            if find_face_in == 0 {
                                faces = VectorOfRect::new();
                                match face_detector.detect_multi_scale(
                                    &gray_buf,
                                    &mut faces,
                                    1.1,
                                    2,
                                    objdetect::CASCADE_SCALE_IMAGE,
                                    Size::new(30, 30),
                                    Size::new(0, 0)
                                ) {
                                    Ok(_) => {},
                                    Err(e) => println!("Error with face detector: {}", e),
                                };

                                find_face_in = 3;
                            }

                            // We can still display the last frames rectangle though, since it's
                            // quite impossible to notice changes in that little amount of time.
                            for face in &faces {
                                rectangle(
                                    &mut bgra_frame,
                                    face,
                                    cv::core::Scalar::new(255.0, 0.0, 0.0, 0.0),
                                    2,
                                    cv::imgproc::LINE_8,
                                    0
                                ).unwrap();
                            }

                            match highgui::imshow("tello", &bgra_frame) {
                                Ok(_) => {},
                                Err(e) => println!("Error: {:?}", e),
                            };

                            highgui::poll_key();

                            match vrx.try_recv() {
                                Ok(v) => {
                                    if v == crate::ThreadMsg::ShutdownThread {
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
    })
}
