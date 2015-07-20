extern crate gl;
extern crate glfw;
extern crate time;
extern crate point;

use std::sync::mpsc::channel;
use std::thread::spawn;
use std::mem;
use std::ptr;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::fmt::{ Display, Formatter };
use std::fmt;
use std::cmp::{Eq, PartialEq};

use gl::types::*;
use glfw::{Context, Key, OpenGlProfileHint, Window, WindowHint, WindowMode};

use point::Point;
mod gl_util;

mod mandel {
    pub const DETAIL : u32 = 128;

    pub fn calc(ox:f64, oy:f64) -> u32 {
        let mut x = ox;
        let mut y = oy;

        for i in 0..DETAIL {
            let xtemp = x*x - y*y + ox;
            y = 2.0*x*y + oy;
            x = xtemp;

            if x*x + y*y > 4.0 {
                return i;
            }
        }

        return DETAIL;
    }
}

struct TileSpecification {
    pixels: Point<i32>,
    center: Point<f64>,
    zoom:   f64,
}

impl PartialEq for TileSpecification {
    fn eq(&self, other: &TileSpecification) -> bool {
        self.pixels == other.pixels &&
        self.center == other.center &&
        self.zoom == other.zoom
    }
}

impl Eq for TileSpecification {}

struct Tile {
    specification: TileSpecification,
    colors:        Vec<GLfloat>,
    positions:     Vec<GLfloat>,
}

struct Line {
    y: i32,
    values: Vec<u32>,
}

struct HumanTimeDuration {
    nanoseconds: u64,
}

impl Display for HumanTimeDuration {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), fmt::Error> {
        let ns = self.nanoseconds;
        match ns {
                        0 ...         1_000 => fmt.write_fmt(format_args!("{} ns", ns)),
                    1_000 ...     1_000_000 => fmt.write_fmt(format_args!("{:.*} µs", 2, (ns as f64) /         1_000f64)),
                1_000_000 ... 1_000_000_000 => fmt.write_fmt(format_args!("{:.*} ms", 2, (ns as f64) /     1_000_000f64)),
                           _                => fmt.write_fmt(format_args!("{:.*} s" , 2, (ns as f64) / 1_000_000_000f64)),
        }
    }
}

// TODO: return result with a useful error type
fn load_shader(filename: &str) -> String {
    let mut file  = File::open(filename)
                        .ok().unwrap_or_else(|| File::open("src/".to_string()+filename)
                                                    .ok().expect(&format!("Could not open shader file {}", filename)));
    let mut bytes = Vec::new();

    file.read_to_end(&mut bytes).ok().expect(&format!("Failed to read from shader file {}", filename));

    String::from_utf8(bytes).ok().expect(&format!("Shader file not UTF-8: {}", filename))
}

fn create_buffer() -> GLuint {
    unsafe {
        let mut buffer = 0;
        gl::GenBuffers(1, &mut buffer);

        buffer
    }
}

unsafe fn load_vector_in_buffer(buffer: u32, values: &Vec<GLfloat>) {
    gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
    gl::BufferData(gl::ARRAY_BUFFER,
                   (values.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
                   mem::transmute(&values[0]),
                   gl::STATIC_DRAW);
}

unsafe fn bind_attribute_to_buffer(program: u32, attribute_name: &str, buffer: u32, components: i32) {
    gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
    let attribute = gl::GetAttribLocation(program, CString::new(attribute_name).unwrap().as_ptr()) as GLuint;
    gl::EnableVertexAttribArray(attribute);
    gl::VertexAttribPointer(attribute, components, gl::FLOAT, gl::FALSE as GLboolean, 0, ptr::null());
}

fn world_width_from_zoom(zoom: f64) -> f64 {
    2f64.powf(zoom)
}

unsafe fn set_viewport(program: GLuint, zoom: f64, pixels: &Point<i32>, center: &Point<f64>) {
    let (world_width, world_height, world_left, _world_top, world_bottom) = get_screen_in_world(zoom, &pixels, &center);
    gl::Uniform2f(gl::GetUniformLocation(program, CString::new("world_bottom_left").unwrap().as_ptr()), world_left  as f32, world_bottom as f32);
    gl::Uniform2f(gl::GetUniformLocation(program, CString::new("world_dimensions" ).unwrap().as_ptr()), world_width as f32, world_height as f32);
}

fn get_screen_in_world(zoom: f64, pixels: &Point<i32>, center: &Point<f64>) -> (f64, f64, f64, f64, f64) {
    let width  = pixels.x as f64;
    let height = pixels.y as f64;
    let world_width   = world_width_from_zoom(zoom);
    let world_height  = world_width * height / width;
    let world_left    = center.x - world_width  / 2.0;
    let world_top     = center.y + world_height / 2.0;
    let world_bottom  = center.y - world_height / 2.0;

    (world_width, world_height, world_left, world_top, world_bottom)
}

fn pixel_to_world(pixel_coord: &Point<f64>, zoom: f64, pixels: &Point<i32>, center: &Point<f64>) -> Point<f64> {
    let (world_width, world_height, world_left, world_top, _world_bottom) = get_screen_in_world(zoom, &pixels, &center);

    Point {
        x:  pixel_coord.x / (pixels.x as f64) * world_width  + world_left,
        y: -pixel_coord.y / (pixels.y as f64) * world_height + world_top,
    }
}

fn calc_mandelbrot(pixels: &Point<i32>, center: &Point<f64>, zoom: f64) -> (Vec<GLfloat>, Vec<GLfloat>) {
    let start = time::precise_time_ns();

    let mut colors    : Vec<GLfloat> = vec![];
    let mut positions : Vec<GLfloat> = vec![];

    let width  = pixels.x as f64;
    let height = pixels.y as f64;

    let (world_width, world_height, world_left, world_top, _world_bottom) = get_screen_in_world(zoom, &pixels, &center);

    let (tx, rx) = channel();
    for y_pixel in 0..pixels.y {

        let tx = tx.clone();
        let x_pixels = pixels.x;

        spawn(move || {
            let mut line = vec![];
            for x_pixel in 0..x_pixels {

                let x =  (x_pixel as f64) / width  * world_width  + world_left;
                let y = -(y_pixel as f64) / height * world_height + world_top;

                let iterations = mandel::calc(x, y);

                line.push(iterations);
            }
            tx.send(Line { y: y_pixel, values: line }).unwrap();
        });
    }

    for _y_pixel in 0..pixels.y {
        let line = rx.recv().unwrap();

        let mut x_pixel = 0;
        for value in line.values {
            x_pixel += 1;
            let y_pixel = line.y;

            positions.push(( (x_pixel as f64) / width  * world_width  + world_left) as f32);
            positions.push((-(y_pixel as f64) / height * world_height + world_top ) as f32);

            let color = value as GLfloat / mandel::DETAIL as GLfloat;
            colors.push(color);
            colors.push(color);
            colors.push(color);
        }
    }

    let end = time::precise_time_ns();
    println!("Calculated fractal in {}", HumanTimeDuration { nanoseconds: end - start });

    (positions, colors)
}

fn draw_fractal(positions : &Vec<GLfloat>, colors : &Vec<GLfloat>, vertex_buffer : GLuint, color_buffer : GLuint, window: &mut Window) {
    let points = colors.len() / 3;

    unsafe {
        load_vector_in_buffer(vertex_buffer, &positions);
        load_vector_in_buffer(color_buffer,  &colors);

        gl::DrawArrays(gl::POINTS, 0, points as i32);

        window.swap_buffers();
    }
}

fn main() {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();

    glfw.window_hint(WindowHint::ContextVersion(3, 2));
    glfw.window_hint(WindowHint::OpenGlForwardCompat(true));
    glfw.window_hint(WindowHint::OpenGlProfile(OpenGlProfileHint::Core));

    let x_initial_points = 500;
    let y_initial_points = 300;

    // since mouse button events don't send mouse positions, we need to store them
    let mut mouse = Point::new(0f64, 0f64);
    let mut mouse_start_pan = Point::new(0f64, 0f64);
    let mut mouse_button_1_pressed = false;

    let mut zoom = 2.0;
    let mut center = Point::new(-0.7, 0.0);

    let (mut window, events) = glfw.create_window(x_initial_points, y_initial_points, "Mandelbrot", WindowMode::Windowed)
        .expect("Failed to create GLFW window.");

    let mut pixels = {
        let (x_pixels, y_pixels) = window.get_framebuffer_size();
        Point::new(x_pixels, y_pixels)
    };

    // on "retina displays" there are two pixels per point, otherwise, it is one
    let pixel_size = pixels.x / (x_initial_points as i32);

    window.set_key_polling(true);
    window.set_framebuffer_size_polling(true);
    window.set_scroll_polling(true);
    window.set_cursor_pos_polling(true);
    window.set_mouse_button_polling(true);
    window.make_current();

    gl::load_with(|s| window.get_proc_address(s));

    let vertex_shader   = gl_util::compile_shader(&load_shader("mandel.v.glsl"), gl::VERTEX_SHADER);
    let fragment_shader = gl_util::compile_shader(&load_shader("mandel.f.glsl"), gl::FRAGMENT_SHADER);
    let program = gl_util::link_program(vertex_shader, fragment_shader);

    unsafe {
        gl::ClearColor(0.0, 0.0, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }

    let mut vertex_array = 0;
    let vertex_buffer = create_buffer();
    let color_buffer = create_buffer();

    unsafe {
        gl::GenVertexArrays(1, &mut vertex_array);
        gl::BindVertexArray(vertex_array);

        gl::UseProgram(program);
        gl::BindFragDataLocation(program, 0, CString::new("out_color").unwrap().as_ptr());

        bind_attribute_to_buffer(program, "position", vertex_buffer, 2);
        bind_attribute_to_buffer(program, "color", color_buffer, 3);
    }

    let mut current_tile : Option<Tile> = None;
    let (tx_incoming_order,  rx_incoming_order ) = channel();
    let (tx_completed_order, rx_completed_order) = channel();

    spawn(move || {
        loop {
            let tile_spec : TileSpecification = rx_incoming_order.recv().unwrap();
            let (positions, colors) = calc_mandelbrot(&tile_spec.pixels, &tile_spec.center, tile_spec.zoom);
            tx_completed_order.send(Tile { specification: tile_spec, positions: positions, colors: colors }).unwrap();
        }
    });

    let mut tile_queue_empty = true;

    while !window.should_close() {
        let mut needs_redraw = false;
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::WindowEvent::Key(Key::Escape, _, _, _) => {
                    window.set_should_close(true)
                }
                glfw::WindowEvent::FramebufferSize(width, height) => {
                    pixels.x = width;
                    pixels.y = height;

                    needs_redraw = true;
                }
                glfw::WindowEvent::Scroll(_x, y) => {
                    let old_world = pixel_to_world(&mouse, zoom, &pixels, &center);

                    zoom += y;

                    let new_world = pixel_to_world(&mouse, zoom, &pixels, &center);

                    center = center + old_world - new_world;

                    needs_redraw = true;
                }
                glfw::WindowEvent::MouseButton(glfw::MouseButton::Button1, glfw::Action::Press, _) => {
                    mouse_button_1_pressed = true;
                    mouse_start_pan = mouse;
                }
                glfw::WindowEvent::MouseButton(glfw::MouseButton::Button1, glfw::Action::Release, _) => {
                    mouse_button_1_pressed = false;
                }
                glfw::WindowEvent::CursorPos(x, y) => {
                    mouse.x = x;
                    mouse.y = y;

                    if mouse_button_1_pressed {
                        let world_per_pixel = world_width_from_zoom(zoom) / (pixels.x as f64);
                        let world_per_point = world_per_pixel * (pixel_size as f64);
                        let mut mouse_movement = mouse - mouse_start_pan;
                        mouse_movement.y = -mouse_movement.y;
                        center = center - mouse_movement * world_per_point;
                        mouse_start_pan = mouse;

                        needs_redraw = true;
                    }

                }
                e => { println!("Unhandled event: {:?}", e); }
            }
        }

        match rx_completed_order.try_recv() {
            Ok(tile) => {
                current_tile = Some(tile);
                tile_queue_empty = true;
                needs_redraw = true;
            },
            _ => {
                // TODO: Handle disconnect
            }
        }

        if needs_redraw {
            unsafe {
                gl::ClearColor(0.2, 0.1, 0.05, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
            }

            unsafe { set_viewport(program, zoom, &pixels, &center) };

            match current_tile {
                Some(ref tile) => {
                    draw_fractal(&tile.positions, &tile.colors, vertex_buffer, color_buffer, &mut window);
                }
                None => { /* no tile ready yet */ }
            }
        }

        let new_tile_spec = TileSpecification { pixels: pixels, center: center, zoom: zoom };

        let needs_new_tile = match current_tile {
            None => true,
            Some(ref tile) => {
                tile.specification != new_tile_spec
            },
        };

        if tile_queue_empty && needs_new_tile {
            tx_incoming_order.send(new_tile_spec).unwrap();
            tile_queue_empty = false;
        }
    }

    unsafe {
        gl::DeleteProgram(program);
        gl::DeleteShader(fragment_shader);
        gl::DeleteShader(vertex_shader);
        gl::DeleteBuffers(1, &color_buffer);
        gl::DeleteBuffers(1, &vertex_buffer);
        gl::DeleteVertexArrays(1, &vertex_array);
    }
}
