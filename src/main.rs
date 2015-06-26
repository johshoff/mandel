extern crate gl;
extern crate glfw;

use std::sync::mpsc::channel;
use std::thread::spawn;
use std::mem;
use std::ptr;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;

use gl::types::*;
use glfw::{Context, Key, OpenGlProfileHint, Window, WindowHint, WindowMode};

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

struct Line {
    y: u32,
    values: Vec<u32>,
}

// TODO: return result with a useful error type
fn load_shader(filename: &str) -> String {
    let mut file  = File::open(filename).ok().expect(&format!("Could not open shader file {}", filename));
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

unsafe fn load_vector_in_buffer(buffer: u32, values: Vec<GLfloat>) {
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

fn calc_mandelbrot(x_pixels: u32, y_pixels: u32, zoom: f64) -> (Vec<GLfloat>, Vec<GLfloat>) {
    println!("Calculating fractal...");

    let mut colors    : Vec<GLfloat> = vec![];
    let mut positions : Vec<GLfloat> = vec![];

    let center_x = -0.7;
    let center_y =  0.0;

    let width  = x_pixels as f64;
    let height = y_pixels as f64;
    let world_width   = 1.0 / zoom;
    let world_height  = 1.0 / zoom * height / width;
    let world_left    = center_x - world_width  / 2.0;
    let _world_right  = center_x + world_width  / 2.0;
    let world_top     = center_y + world_height / 2.0;
    let _world_bottom = center_y - world_height / 2.0;

    let (tx, rx) = channel();
    for y_pixel in 0..y_pixels {

        let tx = tx.clone();

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

    for _y_pixel in 0..y_pixels {
        let line = rx.recv().unwrap();

        let mut x_pixel = 0;
        for value in line.values {
            x_pixel += 1;

            positions.push(x_pixel as GLfloat / x_pixels as GLfloat);
            positions.push(line.y  as GLfloat / y_pixels as GLfloat);

            let color = value as GLfloat / mandel::DETAIL as GLfloat;
            colors.push(color);
            colors.push(color);
            colors.push(color);
        }
    }
    println!("Done");

    (positions, colors)
}

fn draw_fractal(positions : Vec<GLfloat>, colors : Vec<GLfloat>, vertex_buffer : GLuint, color_buffer : GLuint, window: &mut Window) {
    let points = colors.len() / 3;

    unsafe {
        load_vector_in_buffer(vertex_buffer, positions);
        load_vector_in_buffer(color_buffer, colors);

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

    let retina = true;
    let retina_factor = if retina { 2 } else { 1 };

    let mut zoom = 1.0 / 5.5;
    let mut x_pixels = x_initial_points * retina_factor;
    let mut y_pixels = y_initial_points * retina_factor;

    let (mut window, events) = glfw.create_window(x_initial_points, y_initial_points, "Mandelbrot", WindowMode::Windowed)
        .expect("Failed to create GLFW window.");

    window.set_key_polling(true);
    window.set_framebuffer_size_polling(true);
    window.set_scroll_polling(true);
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

    let (positions, colors) = calc_mandelbrot(x_pixels, y_pixels, zoom);
    draw_fractal(positions, colors, vertex_buffer, color_buffer, &mut window);

    while !window.should_close() {
        let mut needs_redraw = false;
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::WindowEvent::Key(Key::Escape, _, _, _) => {
                    window.set_should_close(true)
                }
                glfw::WindowEvent::FramebufferSize(width, height) => {
                    x_pixels = width  as u32;
                    y_pixels = height as u32;

                    needs_redraw = true;
                }
                glfw::WindowEvent::Scroll(_x, y) => {
                    zoom += y * 0.1;

                    needs_redraw = true;
                }
                e => { println!("Unhandled event: {:?}", e); }
            }
        }

        if needs_redraw {
            let (positions, colors) = calc_mandelbrot(x_pixels, y_pixels, zoom);
            draw_fractal(positions, colors, vertex_buffer, color_buffer, &mut window);
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
