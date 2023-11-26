use raylib::prelude::*;
use raylib::core::text::measure_text;
use std::ops::Not;
use std::{
    str,
    thread,
    io::{prelude::*, BufReader},
    net::{TcpListener, TcpStream},
};
const H: i32 = 640;
const W: i32 = 640;
const CELL_H: i32 = H/3;
const CELL_W: i32 = W/3;

const BACKGROUND: Color = Color::BLACK;

const CIRCLE_OUT_R: f32 = (CELL_W/3) as f32;
const CIRCLE_IN_R: f32 = (CELL_W/4) as f32;
const CIRCLE_COLOR: Color = Color::BLUE;

const CROSS_ARM_LEN: f32 = (CELL_W*2/3) as f32;
const CROSS_ARM_THIC: f32 = (CELL_W/6) as f32;
const CROSS_COLOR: Color = Color::RED;

const FONT_SIZE: i32 = 40;
const FONT_COLOR: Color = Color::GREEN;
#[derive(Copy, Clone, PartialEq, Debug)]
enum Player {
    Circle,
    Cross,
}
impl Not for Player {
    type Output = Player;
    fn not(self) -> Self::Output {
        match self {
            Self::Circle => Self::Cross,
            Self::Cross => Self::Circle,
        }
    }
}
#[derive(Copy, Clone, Debug)]
struct Cell {
    content: Option<Player>,
}
#[derive(PartialEq)]
enum Win {
    Complete(Option<Player>),
    Playing,
    Waiting,
}
struct State {
    turn: Player,
    grid: Grid,
    win: Win,
}

impl State {
    fn init() -> Self {
        return State { turn: Player::Circle, grid: [ [Cell { content: None }; 3]; 3 ], win: Win::Waiting };
    }
}
fn create_shape(state: &mut State, x: i32, y: i32) -> bool {
    let cell = &mut state.grid[y as usize][x as usize];
    match cell.content {
        Some(_) => false,
        None => {
            cell.content = Some(state.turn);
            true
        }
    }
    
}

type  Grid = [[Cell; 3]; 3];
fn get_cell_from_pixel<'a>(grid: &'a mut Grid, x: i32, y: i32) -> &'a mut Cell {
    let xc = (x / CELL_W) as usize;
    let yc = (y / CELL_H) as usize;
    return &mut grid[yc][xc];
}
fn get_center_from_cell(xc: i32, yc: i32) -> (i32, i32) {
    return (xc*CELL_W+CELL_W/2, yc*CELL_H+CELL_H/2);
}

fn check_cell(state: &mut State, y: usize, x: usize) {
    if state.grid[x][y].content != Some(state.turn) {
        state.win = Win::Playing;
    }
}

fn check_victory(state: &mut State, new_cx: i32, new_cy: i32) {
    // check horizontal
    state.win = Win::Complete(Some(state.turn));
    for cx in 0..3 {
        check_cell(state, new_cy as usize, cx as usize);
    }
    state.win = Win::Complete(Some(state.turn));
    for cy in 0..3 {
        check_cell(state, cy as usize, new_cx as usize);
    }
    /*
        x
         x
          x
     */
    if new_cx == new_cy {
        state.win = Win::Complete(Some(state.turn));
        for c in 0..3 {
            check_cell(state, c as usize, c as usize);
        }
    }
    /*
          x
         x
        x
     */
    if new_cx == 2-new_cy {
        state.win = Win::Complete(Some(state.turn));
        for c in 0..3 {
            check_cell(state, c as usize, 2-c as usize);
        }
    }
    match state.win {
        Win::Complete(_) => {}
        _ => {
            state.win = Win::Complete(None); // tie
            for row in state.grid {
                for cell in row {
                    if cell.content == None {
                        state.win = Win::Playing;
                    }
                }
            }
        }
    }

}

fn annouce(s: &str, d: &mut RaylibDrawHandle) {
    let start = measure_text(s, FONT_SIZE)/2;
    d.draw_text(s, W/2-start, H/2, FONT_SIZE, FONT_COLOR);
}

fn read_request(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut size_buf = [0; 8];
    stream.read_exact(&mut size_buf).ok()?;
    let size = usize::from_be_bytes(size_buf);
    eprintln!("size: {size}");
    let mut buf = vec![0; size];
    stream.read_exact(buf.as_mut_slice()).ok()?;
    return Some(buf);
}
fn read_coord(stream: &mut TcpStream) -> Option<(i32, i32)> {
    let request = read_request(stream)?;
    let s = std::str::from_utf8(&request.as_slice()).ok()?;
    eprintln!("s: {s}");

    let mut it = s.split(' ');
    let x: i32 = it.next()?.parse().ok()?;
    let y: i32 = it.next()?.parse().ok()?;
    return Some((x,y));
}
fn write_request(stream: &mut TcpStream, bytes: &[u8]) -> Option<usize> {
    let size = bytes.len();
    stream.write_all(&size.to_be_bytes()).ok()?;
    stream.write_all(bytes).ok()?;
    return Some(size);
}
fn write_coord(stream: &mut TcpStream, (x,y): (i32, i32)) -> Option<usize> {
    let s = format!("{x} {y}");
    return write_request(stream, s.as_bytes());
}


const ADDR: &str = "127.0.0.1:7878";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 {
        println!("Error: Please specified whether you are running as sever `-s <ip>` or client `-c <ip>`");
        return;
    }
    let mut ip = String::from(ADDR);
    if args.len() == 2 {
        println!("Warning: No <ip> provided, using {ADDR}");
    }
    if args.len() == 3 {
        ip = args[2].clone();
    }
    
    let server_mode = match &args[1][..] {
        "-s" => true,
        "-c" => false,
        _ => {
            println!("Error: unrecognized command");
            return;
        }
    };
    let mut state = State::init();
    let me = if server_mode { Player::Circle } else { Player::Cross };
    let (tx, rx) = std::sync::mpsc::channel();
    let (tx2, rx2) = std::sync::mpsc::channel();
    
    if server_mode {
        thread::spawn(move || {

            let listener = TcpListener::bind(ip).unwrap();
            let (mut stream, _) = listener.accept().unwrap();
            
            let request = read_request(&mut stream).unwrap();
            let code = std::str::from_utf8(&request.as_slice()).unwrap();
            match code {
                "hello" => {
                    write_request(&mut stream, code.as_bytes());
                    tx.send((-1,-1)).unwrap();
                },
                _ => {
                    println!("Error: Invali Code");
                    return;
                }
            }
            loop {
                let coord = rx2.recv().unwrap();
                write_coord(&mut stream, coord);
                let coord = read_coord(&mut stream).unwrap();
                tx.send(coord).unwrap();
            }

        });
    } else {
        thread::spawn(move || {
            let mut stream = TcpStream::connect(ADDR).expect("failed to connected");

            write_request(&mut stream, "hello".as_bytes());
            let request = read_request(&mut stream).unwrap();
            let code = std::str::from_utf8(&request.as_slice()).unwrap();
            match code {
                "hello" => {
                    tx.send((-1,-1)).unwrap();
                },
                _ => {
                    println!("Error: Invali Code");
                    return;
                }
            }
            loop {
                let coord = read_coord(&mut stream).unwrap();
                println!("coord: {coord:?}");
                tx.send(coord).unwrap();
                let coord = rx2.recv().unwrap();
                write_coord(&mut stream, coord);
            }

            
        });
    }

    let (mut rl, thread) = raylib::init()
        .size(W, H)
        .title("TicTacToe")
        .build();



    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);

        // restart
        if d.is_key_down(KeyboardKey::KEY_R) {
            state = State::init();
        }
        
        // the grid
        d.clear_background(BACKGROUND);
        d.draw_line(0,H/3,W,H/3, Color::WHITE);
        d.draw_line(0,2*H/3,W,2*H/3, Color::WHITE);
        d.draw_line(W/3,0,W/3,H, Color::WHITE);
        d.draw_line(2*W/3,0,2*W/3,H, Color::WHITE);
        match state.turn {
            Player::Circle => d.draw_text("Circle", 0, 0, 15, CIRCLE_COLOR),
            Player::Cross => d.draw_text("Cross", 0, 0, 15, CROSS_COLOR),
        }
        match server_mode {
            true => d.draw_text("server", 0, 15, 15, Color::GREEN),
            false => d.draw_text("client", 0, 15, 15, Color::GREEN),
        }
        
        // new shape
        let px = d.get_mouse_x();
        let py =  d.get_mouse_y();
        if state.win == Win::Waiting {
            match rx.try_recv() {
                Ok((-1,-1)) => {
                    state.win = Win::Playing;
                    println!("INFO: Start Playing!");
                }
                _ => {}
            }
        }
        if state.turn == me && d.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) && state.win == Win::Playing {
            let xc = px / CELL_W;
            let yc = py / CELL_H;
            if create_shape(&mut state, xc, yc) {
                check_victory(&mut state, xc, yc);
                tx2.send((xc, yc)).unwrap();
                state.turn = match state.turn {
                    Player::Circle => Player::Cross,
                    Player::Cross => Player::Circle,
                }
            }
        } 
        else if state.turn == !me && state.win == Win::Playing {
            match rx.try_recv() {
                Ok((xc,yc)) => {
                    if create_shape(&mut state, xc, yc) {
                        println!("coord: {xc} {yc}");

                        check_victory(&mut state, xc, yc);
                        state.turn = match state.turn {
                            Player::Circle => Player::Cross,
                            Player::Cross => Player::Circle,
                        }
                    } else {
                        eprintln!("Error!");
                    }
                },
                _ => {}
            };
        }
        // draw shapes
        for yc in 0..3 {
            for xc in 0..3 {
                let (xp, yp) = get_center_from_cell(xc, yc);
                match state.grid[yc as usize][xc as usize].content {
                    None => {},
                    Some(Player::Circle) => {
                        d.draw_circle(xp, yp, CIRCLE_OUT_R, CIRCLE_COLOR);
                        d.draw_circle(xp, yp, CIRCLE_IN_R, BACKGROUND);
                    }
                    Some(Player::Cross) => {
                        let xpf = xp as f32;
                        let ypf = yp as f32;
                        let hl = CROSS_ARM_LEN/2.0;
                        let ht = CROSS_ARM_THIC/2.0;
                        let half_sqrt = 0.5_f32.sqrt(); // sqrt(1/2) (or sqrt(2)/2)
                        let origin = Vector2 {x: 0., y: 0.};
                        d.draw_rectangle_pro(Rectangle {
                            x: xpf+half_sqrt.sqrt()*(ht-hl),
                            y: ypf+half_sqrt.sqrt()*(-ht-hl),
                            width: CROSS_ARM_LEN*1.2,
                            height: CROSS_ARM_THIC*1.2,
                        }, origin, 45., CROSS_COLOR);
      
                        d.draw_rectangle_pro(Rectangle {
                            x: xpf+half_sqrt.sqrt()*(-ht-hl),
                            y: ypf+half_sqrt.sqrt()*(-ht+hl),
                            width: CROSS_ARM_LEN*1.2,
                            height: CROSS_ARM_THIC*1.2,
                        }, origin, -45., CROSS_COLOR);

                    }
                }
                
            }
        }//draw shapes 
        
        match &state.win {
            Win::Playing => {},
            Win::Waiting => {
                annouce(concat!("Waiting for Oponent ", "127.0.0.1:7878") , &mut d)
            }
            Win::Complete(Some(Player::Circle)) => {
                annouce("Circle wins!", &mut d);
            }
            Win::Complete(Some(Player::Cross)) => {
                annouce("Cross wins!", &mut d);
            }
            Win::Complete(None) => {
                annouce("Tie!", &mut d);
            }
        }
    }
}