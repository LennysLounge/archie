#![allow(unused)]

use std::{
    fs,
    io::{self, Stdout, Write},
    panic,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use archie::{
    isa::{self, Address::*, Instruction::*, Register::*},
    mcu::MCU,
};
use clap::Parser;
use crossterm::{
    ExecutableCommand, QueueableCommand,
    cursor::{DisableBlinking, Hide, MoveTo},
    event::{Event, KeyCode, KeyEvent, KeyEventState, KeyModifiers, poll, read},
    execute, queue,
    style::Print,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, enable_raw_mode},
};
use ux::u4;

#[derive(Parser, Debug)]
struct Cli {
    #[arg(help = "The file to be assembled")]
    input_file: PathBuf,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let program = read_file_as_u16(&cli.input_file)?;

    enable_raw_mode()?;
    get_next_key()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, DisableBlinking, Hide)?;

    // Restore terminal on panic
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let mut stdout = io::stdout();
        stdout.execute(LeaveAlternateScreen).unwrap();
        default_hook(info);
    }));

    let mut mcu = MCU::new(program);
    mcu.set_dbg_flag();

    let mut running = true;
    while running {
        //queue!(stdout, Clear(ClearType::All))?;
        mcu.print_status(&mut stdout);

        let (width, height) = crossterm::terminal::size()?;
        queue!(stdout, MoveTo(0, height - 1), Print("Quit: Q    "),)?;
        if mcu.is_dbg_flag_set() {
            queue!(stdout, Print("Single step: O    Run: R    "))?;
        } else {
            queue!(stdout, Print("Break: B    "))?;
        }
        queue!(stdout, MoveTo(0, 14))?;

        stdout.flush();
        let key = get_next_key()?;
        if let Some(key) = key {
            match key {
                KeyEvent {
                    code: KeyCode::Char('q'),
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    running = false;
                }
                KeyEvent {
                    code: KeyCode::Char('o'),
                    kind: crossterm::event::KeyEventKind::Press,
                    ..
                } => {
                    if mcu.is_dbg_flag_set() {
                        mcu.run_one_cycle();
                    }
                }
                KeyEvent {
                    code: KeyCode::Char('r'),
                    kind: crossterm::event::KeyEventKind::Press,
                    ..
                } => {
                    mcu.unset_dbg_flag();
                }
                KeyEvent {
                    code: KeyCode::Char('b'),
                    kind: crossterm::event::KeyEventKind::Press,
                    ..
                } => {
                    mcu.set_dbg_flag();
                }
                _ => (),
            }
        }
        if !mcu.is_dbg_flag_set() && !mcu.is_halted() {
            let start = Instant::now();
            loop {
                for _ in 0..1000 {
                    mcu.run_one_cycle();
                    if mcu.is_dbg_flag_set() {
                        break;
                    }
                }
                let now = Instant::now();
                if now.duration_since(start) > Duration::from_millis(10)
                    || mcu.is_dbg_flag_set()
                    || mcu.is_halted()
                {
                    break;
                }
            }
        } else {
            thread::sleep(Duration::from_millis(10));
        }
    }
    let (width, height) = crossterm::terminal::size()?;
    execute!(stdout, LeaveAlternateScreen)?;
    Ok(())
}

fn get_next_key() -> io::Result<Option<KeyEvent>> {
    while poll(Duration::from_millis(10))? {
        match read()? {
            Event::Key(event) => return Ok(Some(event)),
            _ => (),
        }
    }
    Ok(None)
}

fn read_file_as_u16(path: &impl AsRef<Path>) -> io::Result<Vec<u16>> {
    let bytes = fs::read(path)?;

    if bytes.len() % 2 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "File length is not a multiple of 2",
        ));
    }

    let vec_u16 = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    Ok(vec_u16)
}
