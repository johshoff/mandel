use std::sync::mpsc::channel;
use std::thread::spawn;


mod mandel {
    pub fn calc(ox:f64, oy:f64) -> u32 {
        let mut x = ox;
        let mut y = oy;
        let detail = 128;

        for i in 0..detail {
            let xtemp = x*x - y*y + ox;
            y = 2.0*x*y + oy;
            x = xtemp;

            if x*x + y*y > 4.0 {
                return i;
            }
        }

        return detail;
    }
}

struct Line {
    y: u32,
    values: Vec<char>,
}

fn main() {
    let x_chars = 200;
    let y_chars = 40;
    let zoom = 1.0 / 5.5;
    let center_x = -0.7;
    let center_y =  0.0;

    let width  = x_chars as f64;
    let height = y_chars as f64;
    let world_width   = 1.0 / zoom;
    let world_height  = 1.0 / zoom * height / width;
    let world_left    = center_x - world_width  / 2.0;
    let _world_right  = center_x + world_width  / 2.0;
    let world_top     = center_y + world_height / 2.0;
    let _world_bottom = center_y - world_height / 2.0;

    let (tx, rx) = channel();
    for y_char in 0..y_chars {

        let tx = tx.clone();

        spawn(move || {
            let mut line = vec![];
            for x_char in 0..x_chars {

                let x =  (x_char as f64) / width  * world_width  + world_left;
                let y = -(y_char as f64) / height * world_height + world_top;

                let iterations = mandel::calc(x, y);

                let symbol : char = match iterations {
                        0        => ' ',
                        1...63   => '+',
                       64...127  => '*',
                      127...255  => '#',
                      _          => 'X',
                };
                line.push(symbol);
            }
            tx.send(Line { y: y_char, values: line }).unwrap();
        });
    }

    for _y_char in 0..y_chars {
        let line = rx.recv().unwrap();
        println!("\x1B[{};0H", line.y);
        for symbol in line.values.into_iter() {
            print!("{}", symbol);
        }
    }
    println!("\x1B[{};0H", y_chars);
    println!("");
}
