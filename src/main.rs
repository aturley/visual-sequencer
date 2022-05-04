use std::cmp;
use std::collections::HashSet;
use std::time::Instant;
use core::ptr::copy;
use core::time::Duration;

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

enum State {
    Idle,
    CreatingRegion(i32, i32)
}

struct Sequencer {
    steps: usize,
    pos: usize,
    ticks: usize,
    ticks_per_step: usize,
    is_updated_flag: bool,
    id: u64
}

impl Sequencer {
    fn new(id: u64) -> Sequencer {
        Sequencer {
            steps: 16,
            pos: 0,
            ticks: 0,
            ticks_per_step: 250,
            is_updated_flag: false,
            id: id
        }
    }

    fn tick_ms(&mut self, ms: u32) {
        self.ticks = self.ticks + (ms as usize);
        if self.ticks >= self.ticks_per_step {
            self.ticks = self.ticks % self.ticks_per_step;
            self.pos = (self.pos + 1) % self.steps;
            self.is_updated_flag = true;
        }
    }

    fn reset(&mut self) {
        self.ticks = 0;
        self.pos = 0;
    }

    fn check_and_reset_is_updated(&mut self) -> bool {
        if self.is_updated_flag {
            self.is_updated_flag = false;
            true
        } else {
            false
        }
    }
}

// in logical units
struct Zone {
    start_x: i32,
    start_y: i32,
    width: u32,
    height: u32,
    sequencer: Sequencer
}

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

    let mut state = State::Idle;

    let mut zones = Vec::<Zone>::new();

    let mut start = Instant::now();

    let mut sequencer_id: u64 = 0;
    
    'running: loop {
        let mouse_state = &mut event_pump.mouse_state();
        let buttons: HashSet<MouseButton> = mouse_state.pressed_mouse_buttons().collect();
        
        for event in event_pump.poll_iter() {
            
            match event {
                Event::Quit {..} |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running;
                } |
                Event::KeyDown { keycode: Some(Keycode::Comma), .. } => {
                    for zone in &mut zones {
                        zone.sequencer.reset();
                    }
                } |
                Event::KeyDown { keycode: Some(Keycode::C), .. } => {
                    match state {
                        State::Idle => {
                            state = State::CreatingRegion(mouse_state.x(), mouse_state.y());
                        } |
                        State::CreatingRegion (start_x, start_y) => {
                            state = State::Idle;
                            let rect_start_x = cmp::min(start_x, mouse_state.x());
                            let rect_start_y = cmp::min(start_y, mouse_state.y());
                            let rect_size_x = cmp::max(start_x, mouse_state.x()) - rect_start_x;
                            let rect_size_y = cmp::max(start_y, mouse_state.y()) - rect_start_y;
                            // convert screen units to logical units
                            zones.push(Zone {start_x: rect_start_x * 2, start_y: rect_start_y * 2, width: (rect_size_x * 2) as u32, height: (rect_size_y * 2) as u32, sequencer: Sequencer::new(sequencer_id)});
                            sequencer_id = sequencer_id + 1;
                        }
                    }
                } |
                Event::KeyDown { keycode: Some(Keycode::X), .. } => {
                    match state {
                        State::CreatingRegion (start_x, start_y) => {
                            state = State::Idle;
                        }
                        _ => {}
                    }
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

        // 
        // analyze the existing zones
        //

        canvas.set_blend_mode(BlendMode::Blend);

        for zone in &mut zones {
            // get the active part of the zone
            let active_width = zone.width / (zone.sequencer.steps as u32);
            let active_start = zone.start_x + ((active_width as i32) * (zone.sequencer.pos as i32));

            canvas.set_draw_color(Color::RGBA(255, 0, 0, 130));
            canvas.fill_rect(Rect::new(active_start, zone.start_y, active_width, zone.height));

            // grab the region of interest from the frame
            let roi = cvcore::Mat::roi(&frame, cvcore::Rect::new(active_start, zone.start_y, active_width as i32, zone.height as i32)).unwrap();

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
            if zone.sequencer.check_and_reset_is_updated() {
                let velocity = (cvcore::count_non_zero(&mask).expect("could not get non zero count") * 127) / (mask.cols() * mask.rows());
                println!("noteon 1 {} {};", zone.sequencer.id, velocity);
            }
        }

        // if a region is being created, draw it
        match state {
            State::CreatingRegion(start_x, start_y) => {
                canvas.set_draw_color(Color::RGBA(0, 0, 255, 50));
                let rect_start_x = cmp::min(start_x, mouse_state.x());
                let rect_start_y = cmp::min(start_y, mouse_state.y());
                let rect_size_x = cmp::max(start_x, mouse_state.x()) - rect_start_x;
                let rect_size_y = cmp::max(start_y, mouse_state.y()) - rect_start_y;
                canvas.fill_rect(Rect::new(rect_start_x * 2, rect_start_y * 2, (rect_size_x * 2) as u32, (rect_size_y * 2) as u32));
            }
            _ => {}
        }

        // draw the existing zones
        for zone in &zones {
            canvas.set_draw_color(Color::RGBA(255, 0, 0, 50));
            canvas.fill_rect(Rect::new(zone.start_x, zone.start_y, zone.width, zone.height));

            // draw the active part of the zone
            let active_width = zone.width / (zone.sequencer.steps as u32);
            let active_start = zone.start_x + ((active_width as i32) * (zone.sequencer.pos as i32));

            canvas.set_draw_color(Color::RGBA(255, 0, 0, 130));
            canvas.fill_rect(Rect::new(active_start, zone.start_y, active_width, zone.height));
        }

        // update the canvas
        canvas.present();

        // update the sequencers
        let elapsed = start.elapsed();
        for zone in &mut zones {
            zone.sequencer.tick_ms(elapsed.as_millis() as u32);
        }
        start = Instant::now();

        // sleep for 1/60th of a second
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}
