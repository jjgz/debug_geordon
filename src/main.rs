#![feature(slice_patterns)]

extern crate serde_json;
extern crate rnet;
extern crate glium;
extern crate glowygraph;

use glium::{Surface, DisplayBuild};
use rnet::Netmessage;

fn main() {
    use std::env::args;
    use std::io::stdin;
    use std::io::{BufRead, BufReader};
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::mpsc::{channel, TryRecvError};
    use std::thread;
    let bindaddress = args()
        .nth(1)
        .unwrap_or_else(|| panic!("Error: Pass an address in the format \"ip:port\" to bind to."));

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
        [[1.0 / hscale, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

        // Finish the render.
        target.finish().unwrap();

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
                    _ => println!("dunno"),
                }
            }
            Err(TryRecvError::Disconnected) => panic!("Input lost."),
            Err(TryRecvError::Empty) => {}
        }
    }
}
