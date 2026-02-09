use std::sync::mpsc;
use pixels::{Error, Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::KeyCode;
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;
use crate::ppu::{WIDTH, HEIGHT};

/// Representation of the application state. In this example, a box will bounce around the screen.

pub struct Renderer {
    rx: mpsc::Receiver<Vec<u8>>,
    tx: mpsc::Sender<Vec<u8>>
}

impl Renderer {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Renderer {
            tx: tx,
            rx: rx
        }
    }

    pub fn get_frame_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.tx.clone()
    }

    pub fn run(self) -> Result<(), Error> {
        let event_loop: EventLoop<()> = EventLoop::new().unwrap();
        let mut input = WinitInputHelper::new();
        let window = {
            let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
            WindowBuilder::new()
                .with_title("Hello Pixels")
                .with_inner_size(size)
                .with_min_inner_size(size)
                .build(&event_loop)
                .unwrap()
        };

        let mut pixels: Pixels<'_> = {
            let window_size = window.inner_size();
            let surface_texture: SurfaceTexture<&winit::window::Window> = SurfaceTexture::new(window_size.width, window_size.height, &window);
            Pixels::new(WIDTH as u32, HEIGHT as u32, surface_texture)?
        };

        let res = event_loop.run(|event, elwt| {
            // Draw the current frame
            if let Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } = event
            {
                // Get frame buffer and render
                if let Err(err) = self.render_frame(&mut pixels) {
                    println!("pixels.render: {}", err.to_string());
                    elwt.exit();
                    return;
                }
            }

            // Handle input events
            if input.update(&event) {
                // Close events
                if input.key_pressed(KeyCode::Escape) || input.close_requested() {
                    elwt.exit();
                    return;
                }

                // Resize the window
                if let Some(size) = input.window_resized() {
                    if let Err(err) = pixels.resize_surface(size.width, size.height) {
                        println!("pixels.resize_surface: {}", err.to_string());
                        elwt.exit();
                        return;
                    }
                }

                // Update internal state and request a redraw
                window.request_redraw();
            }
        });
        res.map_err(|e| Error::UserDefined(Box::new(e)))
    }

    fn render_frame(&self, pixels: &mut Pixels<'_>) -> Result<(), Error> {
        // Get frame buffer and render
        if let Ok(frame) = self.rx.try_recv() {
            let pixels_frame = pixels.frame_mut();
            pixels_frame.clone_from_slice(&frame);
        }
        pixels.render()
    }

}