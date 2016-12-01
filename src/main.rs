#![feature(slice_patterns)]

extern crate serde_json;
extern crate rnet;
extern crate glium;
extern crate glowygraph;
extern crate itertools;

use glium::{Surface, DisplayBuild};
use rnet::{Netmessage, Coordinate};
use glowygraph::render2::Node;

fn main() {
    use std::env::args;
    use std::io::stdin;
    use std::io::{BufRead, BufReader};
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::mpsc::{channel, TryRecvError};
    use std::time;
    use std::thread;
    let bindaddress = args()
        .nth(1)
        .unwrap_or_else(|| panic!("Error: Pass an address in the format \"ip:port\" to bind to."));

    fn dursecond(d: time::Duration) -> f64 {
        let seconds = d.as_secs();
        let nanos = d.subsec_nanos();
        seconds as f64 + nanos as f64 * 0.000000001
    }

    let mut stream = TcpStream::connect::<&str>(&bindaddress).unwrap();
    stream.write(&[42, 72, 69, 76, 76, 79, 42]).unwrap();

    let mut istream = stream.try_clone().unwrap();

    // Spawn a thread to receive Netmessage objects and send them back to the main thread.
    let (msg_sender, msg_receiver) = channel();
    thread::spawn(move || {
        loop {
            let mut header = [0u8; 6];
            let mut body = [0u8; 256];

            istream.read_exact(&mut header).unwrap();
            let body = &mut body[0..header[5] as usize + 1];
            istream.read_exact(body).unwrap();

            if let Ok(m) = serde_json::from_slice(body) {
                msg_sender.send(m).unwrap();
            } else {
                println!("Invalid message.");
            }
        }
    });

    // Spawn a thread to get lines from the stdin and send them back to the main thread.
    let (input_sender, input_receiver) = channel();
    thread::spawn(move || {
        for line in BufReader::new(stdin()).lines() {
            match line {
                Ok(line) => input_sender.send(line).unwrap(),
                Err(e) => panic!("Unable to read line: {}", e),
            }
        }
    });

    let display = glium::glutin::WindowBuilder::new().with_vsync().build_glium().unwrap();
    let glowy = glowygraph::render2::Renderer::new(&display);
    let mut difficulty_grid = vec![99u8; 128 * 128];

    let mut row_requests = (0usize..0).peekable();
    let mut row_request_beginning = time::Instant::now();
    let mut row_request_last = time::Instant::now();

    let mut last_ping_time = time::Instant::now();

    let mut debug_angle: f64 = 0.0;
    let mut debug_x: f64 = 2.5;
    let mut debug_y: f64 = 2.5;

    loop {
        // Get window dimensions.
        let dims = display.get_framebuffer_dimensions();
        // Multiply this by width coordinates to get normalized screen coordinates.
        let hscale = dims.1 as f32 / dims.0 as f32;
        // Get the render target.
        let mut target = display.draw();
        // Clear the screen.
        target.clear_color(0.0, 0.0, 0.0, 1.0);
        // Compute the projection matrix.
        let projection =
        [[1.0 * hscale, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

        // Add all the grid locations.
        glowy.render_nodes(&mut target, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                           projection, &difficulty_grid.iter().enumerate().map(|(i, e)| {
            let x = i % 128;
            let y = i / 128;

            Node{
                position: [x as f32 / 64.0 - 1.0 + 1.0 / 128.0, y as f32 / 64.0 - 1.0 + 1.0 / 128.0],
                inner_color: [1.0, 0.0, 0.0, *e as f32 / 100.0],
                falloff: 0.5,
                falloff_color: [1.0, 0.0, 0.0, *e as f32 / 100.0],
                falloff_radius: 1.0/128.0,
                inner_radius: 0.0,
            }
        }).collect::<Vec<_>>());

        // Render the looking direction.
        glowy.render_edges_round(&mut target, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                           projection, &[Node{
                position: [debug_x as f32 / 5.0 * 2.0 - 1.0, debug_y as f32 / 5.0 * 2.0 - 1.0],
                inner_color: [0.0, 1.0, 0.0, 1.0],
                falloff: 0.5,
                falloff_color: [0.0, 1.0, 0.0, 1.0],
                falloff_radius: 1.0/128.0,
                inner_radius: 0.0,
            }, Node{
                position: [(debug_x + debug_angle.cos()) as f32 / 5.0 * 2.0 - 1.0,
                    (debug_y + debug_angle.sin()) as f32 / 5.0 * 2.0 - 1.0],
                inner_color: [0.0, 1.0, 0.0, 1.0],
                falloff: 0.5,
                falloff_color: [0.0, 1.0, 0.0, 1.0],
                falloff_radius: 1.0/128.0,
                inner_radius: 0.0,
            }]);

        // Finish the render.
        target.finish().unwrap();

        for ev in display.poll_events() {
            use glium::glutin::{Event, ElementState, VirtualKeyCode as VKC};
            match ev {
                Event::Closed => return,
                Event::KeyboardInput(ElementState::Pressed, _, Some(VKC::Right)) => {
                    debug_angle -= 0.1;
                    serde_json::to_writer(&mut stream, &Netmessage::Movement(
                        rnet::Point{
                            x: debug_x,
                            y: debug_y,
                            v: 0.0,
                            angle: debug_angle,
                            av: 0.0,
                        }
                    )).unwrap();
                }
                Event::KeyboardInput(ElementState::Pressed, _, Some(VKC::Left)) => {
                    debug_angle += 0.1;
                    serde_json::to_writer(&mut stream, &Netmessage::Movement(
                        rnet::Point{
                            x: debug_x,
                            y: debug_y,
                            v: 0.0,
                            angle: debug_angle,
                            av: 0.0,
                        }
                    )).unwrap();
                }
                _ => {}
            }
        }

        if let Some(n) = row_requests.peek() {
            // If we havent heard back in some time.
            if time::Instant::now() - row_request_last > time::Duration::from_millis(5000) {
                // Resend the request.
                serde_json::to_writer(&mut stream,
                                      &Netmessage::GDReqHalfRow(*n as u8)).unwrap();
                row_request_last = time::Instant::now();
            }
        }

        // Handle network messages.
        match msg_receiver.try_recv() {
            Ok(m) => {
                // Match the Netmessage type.
                match m {
                    Netmessage::ReqName => {
                        serde_json::to_writer(&mut stream, &Netmessage::NameDebugGeordon).unwrap();
                    }
                    Netmessage::Heartbeat => {}
                    Netmessage::ReqNetstats => {}
                    Netmessage::GDPing => {
                        println!("Ping round-trip time: {} seconds",
                                 dursecond(time::Instant::now() - last_ping_time));
                    }
                    Netmessage::GDHalfRow(v) => {
                        if let Some(n) = row_requests.next() {
                            use itertools::Itertools;
                            row_request_last = time::Instant::now();
                            difficulty_grid.chunks_mut(64).nth(n as usize).unwrap().iter_mut().set_from(v);
                            // Check if this was the last one.
                            if row_requests.peek().is_none() {
                                // Since it was, print out the duration.
                                let difference = row_request_last - row_request_beginning;
                                println!("Half-row request fulfilled after {} seconds.",
                                         dursecond(difference));
                            } else {
                                // More rows are to be requested.
                                serde_json::to_writer(&mut stream,
                                   &Netmessage::GDReqHalfRow(*row_requests.peek().unwrap() as u8)).unwrap();
                            }
                        } else {
                            println!("Warning: Got a half-row when none were requested.");
                        }
                    }
                    Netmessage::Movement(p) => {
                        debug_x = p.x;
                        debug_y = p.y;
                        debug_angle = p.angle;
                    }
                    Netmessage::DebugGeordon(s) => {
                        println!("Debug message: {}", s);
                    }
                    _ => println!("Unhandled message: {:?}", m),
                }
            }
            Err(TryRecvError::Disconnected) => panic!("Connection lost."),
            Err(TryRecvError::Empty) => {}
        }

        // Handle input from terminal.
        match input_receiver.try_recv() {
            Ok(line) => {
                let words = line.split(' ').collect::<Vec<_>>();
                // Match the String.
                match words.as_slice() {
                    &["move", x, y, v, angle, av] => {
                        serde_json::to_writer(&mut stream, &Netmessage::Movement(
                            rnet::Point{
                                x: x.parse().unwrap(),
                                y: y.parse().unwrap(),
                                v: v.parse().unwrap(),
                                angle: angle.parse().unwrap(),
                                av: av.parse().unwrap(),
                            }
                        )).unwrap();
                    }
                    &["move", ..] => {
                        println!("Usage: move x y v angle av");
                    }
                    &["turn", angle] => {
                        let ang: f64 = angle.parse().unwrap();
                        debug_angle += ang;
                        serde_json::to_writer(&mut stream, &Netmessage::Movement(
                            rnet::Point{
                                x: debug_x,
                                y: debug_y,
                                v: 0.0,
                                angle: debug_angle,
                                av: 0.0,
                            }
                        )).unwrap();
                    }
                    &["turn", ..] => {
                        println!("Usage: turn angle");
                    }
                    &["rows", n] => {
                        let n: usize = n.parse().unwrap();
                        row_requests = (n..n+1).peekable();
                        row_request_beginning = time::Instant::now();

                        serde_json::to_writer(&mut stream,
                              &Netmessage::GDReqHalfRow(n as u8)).unwrap();
                        row_request_last = time::Instant::now();
                    }
                    &["rows", n, m] => {
                        let n: usize = n.parse().unwrap();
                        let m: usize = m.parse().unwrap();
                        if n < m {
                            row_requests = (n..m).peekable();
                            row_request_beginning = time::Instant::now();

                            serde_json::to_writer(&mut stream,
                                                  &Netmessage::GDReqHalfRow(n as u8)).unwrap();
                            row_request_last = time::Instant::now();
                        } else {
                            println!("Error: The first argument to \"rows\" must be less than the second.");
                        }
                    }
                    &["rows", ..] => {
                        println!("Usage: rows [start] [end]");
                    }
                    &["fakerow"] => {
                        serde_json::to_writer(&mut stream,
                                              &Netmessage::GDHalfRow(vec![0; 64])).unwrap();
                    }
                    &["ping"] => {
                        serde_json::to_writer(&mut stream,
                                              &Netmessage::GDReqPing).unwrap();
                        last_ping_time = time::Instant::now();
                    }
                    &["finish"] => {
                        serde_json::to_writer(&mut stream,
                                              &Netmessage::GDFinish).unwrap();
                    }
                    &["aligned"] => {
                        serde_json::to_writer(&mut stream,
                                              &Netmessage::GDAligned).unwrap();
                    }
                    &["init", nt, x, y, ref borders..] if borders.len() % 2 == 0 => {
                        let acx = x.parse().unwrap();
                        let acy = y.parse().unwrap();
                        debug_x = acx;
                        debug_y = acy;
                        debug_angle = 0.0;
                        let v = borders.chunks(2).map(|s| Coordinate{
                            x: s[0].parse().unwrap(),
                            y: s[1].parse().unwrap(),
                        }).collect::<Vec<_>>();
                        serde_json::to_writer(&mut stream,
                                              &Netmessage::Initialize{
                                                    nt: nt.parse().unwrap(),
                                                  ra: Coordinate{
                                                      x: acx,
                                                      y: acy,
                                                  },
                                                  bd: v,
                                              }).unwrap();
                        last_ping_time = time::Instant::now();
                    }
                    &["build"] => {
                        serde_json::to_writer(&mut stream,
                                              &Netmessage::GDBuild).unwrap();
                        // Also clear the grid locally
                        for e in &mut difficulty_grid {
                            *e = 99;
                        }
                    }
                    &["init", ..] => {
                        println!("Usage: init num_targets x y [border_x border_y]..");
                    }
                    _ => println!("Commands: move, rows, fakerow, ping, init, build, finish, aligned, turn"),
                }
            }
            Err(TryRecvError::Disconnected) => panic!("Input lost."),
            Err(TryRecvError::Empty) => {}
        }
    }
}
