use std::{
    collections::HashMap,
    fmt::format,
    io::{self, Write},
    net::SocketAddr,
};

#[tokio::main]
async fn main() {
    let mut stdout_l = io::stdout().lock();

    let lines = io::stdin().lines();
    write!(stdout_l, "> ").unwrap();
    stdout_l.flush().unwrap();

    for line in lines {
        match line {
            Ok(line) => {
                let mut split = line.split_whitespace();
                let cmd = split.next().unwrap();
                let args: Vec<&str> = split.collect();

                handle_command(cmd, &args);
            }
            Err(e) => println!("Error reading line: {e}"),
        };

        write!(stdout_l, "> ").unwrap();
        stdout_l.flush().unwrap();
    }
}

fn handle_command(command: &str, args: &[&str]) {
    match command {
        "connect" => handle_command_connect(args),
        _ => display_help(),
    }
}

fn handle_command_connect(args: &[&str]) {
    let addr = match args.len() {
        1 => args.first().expect("no ip:port provided").parse(),
        2 => {
            let ip = args.first().expect("no ip provided to connect");
            let port = args.get(1).expect("no port provided to connect");
            format!("{ip}:{port}").parse()
        }
        _ => todo!(),
    };

    handle_command_connect_impl(addr.expect("failed to parse as SocketAddr"));
}

fn handle_command_connect_impl(addr: SocketAddr) {
    println!("{addr:#?}");
}

fn handle_command_list(args: &[&str]) {}

fn display_help() {
    println!(
        r"
Available commands:
    connect <ip|hostname>:<port> - Connect to the IP or hostname
    list <path> - list the files at the given path
"
    )
}
