
use opencv as cv;
use cv::{
    highgui,
    objdetect::{CascadeClassifier, self},
    types::VectorOfRect, prelude::CascadeClassifierTrait,
    core::Size,
    imgproc::rectangle,
    imgproc::circle,
};

mod decoding;
mod conversions;

const OUTER_MOVEMENT_TOLERANCE: i32 = 200;
const INNER_MOVEMENT_TOLERANCE: i32 = OUTER_MOVEMENT_TOLERANCE - 80;

fn check_if_rect_in_rect(outer_rect: cv::core::Rect, inner_rect: cv::core::Rect) -> bool {
    let rect_pts = [
        cv::core::Point::new(inner_rect.x, inner_rect.y),
        cv::core::Point::new(inner_rect.x + inner_rect.width, inner_rect.y),
        cv::core::Point::new(inner_rect.x, inner_rect.y + inner_rect.height),
        cv::core::Point::new(inner_rect.x + inner_rect.width, inner_rect.y + inner_rect.height),
    ];

    let mut result = true;

    let mut truecount = 0;
    let mut falsecount = 0;

    for point in rect_pts {
        match outer_rect.contains(point) {
            true => {
                truecount += 1;
                if (truecount > 1) {
                    return true;
                }
            },
            false => {
                falsecount += 1;
                result &= false;
            }
        }
    }

    return result;
}

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
        // Limit the classifier to only around 8 fps for performance.
        let mut find_face_in = 3;

        let mut last_face: Option<cv::core::Rect> = None;

        let mut no_face_since = 5;

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

                                no_face_since -= 1;

                                match last_face {
                                    None => {
                                        let mut biggest_face = cv::core::Rect::new(0, 0, 0, 0);
                                        for face in &faces {
                                            let biggest_perim = (2 * biggest_face.width) + (2 * biggest_face.height);
                                            let face_perim = (2 * face.width) + (2 * face.height);

                                            if biggest_perim <= face_perim {
                                                biggest_face = face;
                                            }
                                        }

                                        if biggest_face != cv::core::Rect::new(0, 0, 0, 0) {
                                            last_face = Some(biggest_face);
                                            no_face_since = 5;
                                        }
                                    },
                                    Some(lface) => {
                                        let mut closest_face = (cv::core::Rect::new(0, 0, 0, 0), std::i32::MAX);

                                        for face in &faces {
                                            let pos_difference = (lface.x - face.x).abs() + (lface.y - face.y).abs();
                                            let wh_difference = (lface.width - face.width).abs() + (lface.height - face.height).abs();

                                            let diff_score = pos_difference + wh_difference;

                                            // Display all possibilities with green
                                            rectangle(
                                                &mut bgra_frame,
                                                face,
                                                cv::core::Scalar::new(0.0, 255.0, 0.0, 0.0),
                                                2,
                                                cv::imgproc::LINE_8,
                                                0
                                            ).unwrap();

                                            if diff_score < closest_face.1 {
                                                closest_face = (face, diff_score);
                                            }
                                        }

                                        if closest_face.1 != std::i32::MAX {
                                            last_face = Some(closest_face.0);
                                            no_face_since = 5;
                                        }
                                    }
                                }

                                find_face_in = 3; // frames
                            }

                            let screen_middle = (((result.1).0 / 2) as i32, ((result.1).1 / 2) as i32);

                            // Middle point in which it is fine to take place for the drone and the face boundaries, 
                            // if all points are out of this the drone needs to go further back.
                            let mid_screen_bounds = cv::core::Rect::new(
                                screen_middle.0 - OUTER_MOVEMENT_TOLERANCE,
                                screen_middle.1 - OUTER_MOVEMENT_TOLERANCE,
                                2 * OUTER_MOVEMENT_TOLERANCE,
                                2 * OUTER_MOVEMENT_TOLERANCE
                            );

                            rectangle(
                                &mut bgra_frame,
                                mid_screen_bounds,
                                cv::core::Scalar::new(255.0, 255.0, 255.0, 0.0),
                                2,
                                cv::imgproc::LINE_8,
                                0,
                            ).unwrap();

                            if no_face_since == 0 {
                                last_face = None;
                            }

                            // We can still display the last frames rectangle though, since it's
                            // quite impossible to notice changes in that little amount of time.
                            if last_face != None {
                                let lface = last_face.unwrap();
                                let face_mid_point = cv::core::Point::new(lface.x + (lface.width / 2), lface.y + (lface.height / 2));

                                if !mid_screen_bounds.contains(face_mid_point) {
                                    println!("move accordingly")
                                }

                                let outer_movement_boundaries = cv::core::Rect::new(
                                    face_mid_point.x - OUTER_MOVEMENT_TOLERANCE,
                                    face_mid_point.y - OUTER_MOVEMENT_TOLERANCE,
                                    2 * OUTER_MOVEMENT_TOLERANCE,
                                    2 * OUTER_MOVEMENT_TOLERANCE
                                );

                                // TODO: Define a good inner movement tolerance
                                let inner_movement_boundaries = cv::core::Rect::new(
                                    face_mid_point.x - INNER_MOVEMENT_TOLERANCE,
                                    face_mid_point.y - INNER_MOVEMENT_TOLERANCE,
                                    2 * INNER_MOVEMENT_TOLERANCE,
                                    2 * INNER_MOVEMENT_TOLERANCE
                                );

                                rectangle(
                                    &mut bgra_frame,
                                    outer_movement_boundaries,
                                    cv::core::Scalar::new(0.0, 0.0, 255.0, 0.0),
                                    2,
                                    cv::imgproc::LINE_8,
                                    0
                                ).unwrap();

                                rectangle(
                                    &mut bgra_frame,
                                    inner_movement_boundaries,
                                    cv::core::Scalar::new(0.0, 0.0, 255.0, 0.0),
                                    2,
                                    cv::imgproc::LINE_8,
                                    0
                                ).unwrap();

                                if check_if_rect_in_rect(outer_movement_boundaries, lface) {
                                    println!("cool");
                                    if check_if_rect_in_rect(inner_movement_boundaries, lface) {
                                        println!("go forward");
                                    }
                                } else {
                                    println!("go back");
                                }

                                rectangle(
                                    &mut bgra_frame,
                                    lface,
                                    cv::core::Scalar::new(255.0, 0.0, 0.0, 0.0),
                                    2,
                                    cv::imgproc::LINE_8,
                                    0
                                ).unwrap();

                                circle(
                                    &mut bgra_frame,
                                    face_mid_point,
                                    10,
                                    cv::core::Scalar::new(0.0, 0.0, 255.0, 0.0),
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
