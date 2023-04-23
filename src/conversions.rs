
use opencv as cv;
use cv::{
    prelude::Mat,
    imgproc::{cvt_color, COLOR_RGBA2BGRA, COLOR_BGRA2GRAY},
};

use libc::c_void;

pub fn vec_to_bgra_mat(rgba_vec: &mut Vec<u8>, dims: (usize, usize)) -> Result<Mat, ()> {
    let mut bgra_buffer = unsafe { Mat::new_rows_cols(dims.1 as i32, dims.0 as i32, cv::core::CV_8UC4).unwrap() };

    let frame = unsafe { match Mat::new_rows_cols_with_data(
            dims.1 as i32,
            dims.0 as i32,
            cv::core::CV_8UC4,
            rgba_vec.as_mut_ptr() as *mut c_void,
            0,
        ) {
            Ok(m) => m,
            Err(e) => {
                println!("Error with creating frame for conversion: {}", e);
                return Err(());
            }
        }
    };
    
    match cvt_color(&frame, &mut bgra_buffer, COLOR_RGBA2BGRA, 0) {
        Ok(_) => {},
        Err(e) => {
            println!("Error with colour conversion: {}", e);
            return Err(());
        },
    };
    
    Ok(bgra_buffer)
}

pub fn mat_to_grayscale(matrix: &Mat, dims: (usize, usize)) -> Result<Mat, ()> {
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
