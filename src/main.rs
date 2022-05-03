use std::collections::HashSet;
use core::ptr::copy;

use opencv::prelude::*;
use opencv::core as cvcore;
use opencv::imgproc;
use opencv::Result;
use opencv::videoio;

use sdl2::pixels::Color;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::event::Event;
use sdl2::event::WindowEvent;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::render::BlendMode;

use std::time::Duration;

fn main() -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    let window = video_subsystem.window("rust-sdl2 demo", 800, 600)
        .position_centered()
        .opengl()
        .resizable()
        .build()
        .expect("could not initialize video subsystem");

    #[cfg(ocvrs_opencv_branch_32)]
    let mut cam = videoio::VideoCapture::new_default(1).expect("Failed to get camera"); // 0 is the default camera
    #[cfg(not(ocvrs_opencv_branch_32))]
    let mut cam = videoio::VideoCapture::new(1, videoio::CAP_ANY).expect("Failed to get camera"); // 0 is the default camera
    let opened = videoio::VideoCapture::is_opened(&cam).expect("Failed to open camera");
    if !opened {
	panic!("Unable to open default camera!");
    }

    let cam_height = videoio::VideoCapture::get(&cam, videoio::CAP_PROP_FRAME_HEIGHT).expect("failed to get height") as u32;
    let cam_width = videoio::VideoCapture::get(&cam, videoio::CAP_PROP_FRAME_WIDTH).expect("failed to get width") as u32;

    let mut canvas = window.into_canvas().build()
        .expect("could not make a canvas");

    let texture_creator = canvas.texture_creator();

    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::BGR24, cam_width, cam_height)
        .map_err(|e| e.to_string())?;

    // Create a red-green gradient
    texture.with_lock(None, |buffer: &mut [u8], pitch: usize| {
        for y in 0..256 {
            for x in 0..256 {
                let offset = y * pitch + x * 3;
                buffer[offset] = x as u8;
                buffer[offset + 1] = y as u8;
                buffer[offset + 2] = 0;
            }
        }
    })?;

    canvas.set_draw_color(Color::RGB(0, 255, 255));
    canvas.clear();
    canvas.copy(&texture, None, Some(Rect::new(100, 100, 256, 256)))?;
    canvas.present();

    let mut event_pump = sdl_context.event_pump()?;
    let mut i = 0;

    'running: loop {
        i = (i + 1) % 255;
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running;
                } |
                Event::Window {win_event, .. } => {
                    match win_event {
                        WindowEvent::Resized(w, h) => {}
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        // The rest of the game loop goes here...

        let state = event_pump.mouse_state();
        let buttons: HashSet<MouseButton> = state.pressed_mouse_buttons().collect();

        if !buttons.is_empty() {
            println!("X = {:?}, Y = {:?}", state.x(), state.y());
        }

        canvas.set_draw_color(Color::RGB(0, 255, 255));
        canvas.clear();

        // get the frame from the camera
        let mut frame = Mat::default();
	cam.read(&mut frame).expect("Failed to read frame");
	if frame.size().expect("No size").width <= 0 {
            println!("no image yet")
        }

        // get the underlying c pointer to the data
        let buffer = frame.data();
        let size = frame.size().expect("error getting frame size");
        let s = ((size.width * size.height) as usize) * frame.elem_size().expect("error getting frame element size");

        // copy the data into a Rust vec
        let mut vec = Vec::with_capacity(s);
        unsafe {
            vec.set_len(s);
            copy(buffer, vec.as_mut_ptr(), s);
        }

        // update the texture that displays the image with the frame data
        texture.update(Some(Rect::new(0, 0, size.width as u32, size.height as u32)), &vec, frame.elem_size().expect("error getting frame element size") * (frame.cols() as usize)).expect("failed to update the texture");

        canvas.set_scale(0.5, 0.5);
        canvas.copy(&texture, None, Some(Rect::new(0, 0, size.width as u32, size.height as u32)))?;

        // grab the region of interest from the frame
        let roi = cvcore::Mat::roi(&frame, cvcore::Rect::new(200, 200, 200, 200)).unwrap();

        let mut bgr2hsv_image = cvcore::Mat::default();

        // convert the image from the camera to an HSV image
        imgproc::cvt_color(
	    &roi,
	    &mut bgr2hsv_image,
	    imgproc::COLOR_BGR2HSV,
	    0,
	).expect("could not do color conversion");

        // look for regions of red in the image. the "H" in "HSV" stands for "hue", red is in the lower range of values.
        let lower = cvcore::Scalar::new(0.0, 25.0, 0.0, 255.0);
        let upper = cvcore::Scalar::new(15.0, 255.0, 255.0 , 255.0);

        let mut mask = cvcore::Mat::default();
        
        cvcore::in_range(&bgr2hsv_image, &lower, &upper, &mut mask);

        // count how many red vs not red pixels are in the image
        let mut is_color: usize = 0;
        let mut is_not_color: usize = 0;

        for i in 0..(mask.cols() * mask.rows()) {
            if mask.at::<u8>(i).expect("fuuuuck") > &0 {
                is_color = is_color + 1;
            } else {
                is_not_color = is_not_color + 1;
            }
        }

        // if the majority of pixels are red, draw an opaque red box over the region, other draw a transparent red box
        canvas.set_blend_mode(BlendMode::Blend);
        if is_color > is_not_color {
            canvas.set_draw_color(Color::RGBA(255, 0, 0, 127));
        } else {
            canvas.set_draw_color(Color::RGBA(255, 0, 0, 255));
        }
        canvas.fill_rect(Rect::new(200, 200, 200, 200));

        // update the canvas
        canvas.present();

        // sleep for 1/60th of a second
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}
