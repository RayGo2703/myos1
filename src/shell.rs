use crate::vga_buffer;
use crate::{print, println};
use lazy_static::lazy_static;
use pc_keyboard::DecodedKey;
use spin::Mutex;

const MAX_LINE_LENGTH: usize = 64;

#[derive(Clone, Copy)]
struct ShellState {
    buffer: [u8; MAX_LINE_LENGTH],
    length: usize,
    ready: bool,
}

impl ShellState {
    const fn new() -> Self {
        Self {
            buffer: [0; MAX_LINE_LENGTH],
            length: 0,
            ready: false,
        }
    }

    fn push_byte(&mut self, byte: u8) {
        if self.length >= MAX_LINE_LENGTH {
            return;
        }

        self.buffer[self.length] = byte;
        self.length += 1;
    }

    fn pop_byte(&mut self) {
        if self.length == 0 {
            return;
        }

        self.length -= 1;
    }

    fn take_line(&mut self) -> Option<LineBuffer> {
        if !self.ready {
            return None;
        }

        let mut line = LineBuffer::new();
        line.length = self.length;
        line.bytes[..self.length].copy_from_slice(&self.buffer[..self.length]);
        self.length = 0;
        self.ready = false;
        Some(line)
    }
}

#[derive(Clone, Copy)]
pub struct LineBuffer {
    bytes: [u8; MAX_LINE_LENGTH],
    length: usize,
}

impl LineBuffer {
    const fn new() -> Self {
        Self {
            bytes: [0; MAX_LINE_LENGTH],
            length: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.length]).unwrap_or("")
    }
}

pub enum Command<'a> {
    Empty,
    Help,
    Echo(&'a str),
    Clear,
    RunTasks,
    Tasks,
    Unknown(&'a str),
}

lazy_static! {
    static ref SHELL_STATE: Mutex<ShellState> = Mutex::new(ShellState::new());
}

pub fn run() -> ! {
    print_prompt();

    loop {
        if let Some(line) = take_line() {
            execute_line(line.as_str());
            print_prompt();
        }

        x86_64::instructions::hlt();
    }
}

pub fn handle_key(key: DecodedKey) {
    match key {
        DecodedKey::Unicode('\n') | DecodedKey::Unicode('\r') => {
            submit_line();
            // process the line immediately after Enter
            if let Some(line) = take_line() {
                execute_line(line.as_str());
                print_prompt();
            }
        }
        DecodedKey::Unicode('\u{8}') | DecodedKey::Unicode('\u{7f}') => {
            erase_character()
        }
        DecodedKey::Unicode(character) 
            if character.is_ascii() && !character.is_control() => {
            append_character(character)
        }
        _ => {}
    }
}

pub fn parse_command(input: &str) -> Command<'_> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Command::Empty;
    }

    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let command = parts.next().unwrap_or("");
    let arguments = parts.next().unwrap_or("").trim_start();

    match command {
        "help" => Command::Help,
        "clear" => Command::Clear,
        "echo" => Command::Echo(arguments),
        "run" if arguments == "tasks" => Command::RunTasks,
        "tasks" => Command::Tasks,
        _ => Command::Unknown(trimmed),
    }
}

pub fn execute_line(line: &str) {
    match parse_command(line) {
        Command::Empty => {}
        Command::Help => {
            println!("Available commands:");
            println!("  help  - show this message");
            println!("  echo  - print the rest of the line");
            println!("  clear - clear the screen");
            println!("  tasks - show info about the task scheduler");
        }
        Command::Echo(text) => println!("{}", text),
        Command::Clear => {
            vga_buffer::WRITER.lock().clear_screen();
        }
        Command::RunTasks => {
            println!("Starting task scheduler...");

            crate::task::scheduler::request_spawn();
        }
        
        Command::Tasks => {
            println!("Task Scheduler Info: ");
            println!(" Type 'run tasks' to spawn 5 demo tasks.");
            println!(" Tasks run cooperatively using async/await.");
            println!(" Each task yields between steps (round robin).");
        }
        
        Command::Unknown(command) => {
            println!("Unknown command: {}", command);
            println!("Type 'help' for a list of commands.");
        }
    }
}

fn print_prompt() {
    print!("myos> ");
}

fn append_character(character: char) {
    let mut shell = SHELL_STATE.lock();
    if shell.ready {
        return;
    }

    shell.push_byte(character as u8);
    print!("{}", character);
}

fn erase_character() {
    let mut shell = SHELL_STATE.lock();
    if shell.ready {
        return;
    }

    shell.pop_byte();
    vga_buffer::WRITER.lock().backspace();
}

fn submit_line() {
    let mut shell = SHELL_STATE.lock();
    if shell.ready {
        return;
    }

    println!("");
    shell.ready = true;
}

fn take_line() -> Option<LineBuffer> {
    SHELL_STATE.lock().take_line()
}

#[cfg(test)]
#[test_case]
fn parse_help_command() {
    assert!(matches!(parse_command("help"), Command::Help));
}

#[cfg(test)]
#[test_case]
fn parse_echo_command() {
    assert!(matches!(parse_command("echo hello"), Command::Echo("hello")));
}

#[cfg(test)]
#[test_case]
fn parse_unknown_command() {
    assert!(matches!(parse_command("status"), Command::Unknown("status")));
}

#[cfg(test)]
#[test_case]
fn parse_run_tasks_command() {
    assert!(matches!(parse_command("run tasks"), Command::RunTasks));
}
