#![allow(unused)]

use std::{
    io::{self, Stdout, Write},
    panic, thread,
    time::{Duration, Instant},
};

use archie::{
    isa::{self, Address::*, Instruction::*, Register::*},
    mcu::MCU,
};
use crossterm::{
    ExecutableCommand, QueueableCommand,
    cursor::{DisableBlinking, Hide, MoveTo},
    event::{Event, KeyCode, KeyEvent, KeyEventState, KeyModifiers, poll, read},
    execute, queue,
    style::Print,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, enable_raw_mode},
};
use ux::u4;

fn main() -> io::Result<()> {
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

    let program = vec![
        // setup variables
        LDI_POS(R1, u4::new(1)),
        LDI_POS(R2, u4::new(3)),
        LDI_POS(R3, u4::new(5)),
        LDI_IMM(R7, 1000),
        //
        DECB(R2),
        JNE(2),
        MOV(R4, R1),
        LDI_POS(R2, u4::new(3)),
        DECB(R3),
        JNE(2),
        MOV(R4, R1),
        LDI_POS(R3, u4::new(5)),
        ADD(R5, R4),
        ADDC(R6, R0),
        MOV(R4, R0),
        INCB(R1),
        CMP(R1, R7),
        JL(-14),
    ]; // 11059

    let program = vec![
        // setup variables
        LDI_POS(R1, u4::new(1)),
        LDI_POS(R2, u4::new(3)),
        LDI_POS(R3, u4::new(5)),
        LDI_IMM(R7, 1000),
        //
        DECB(R2),
        JNE(7),
        ADD(R5, R1),
        ADDC(R6, R0),
        LDI_POS(R2, u4::new(3)),
        DECB(R3),
        JNE(1),
        LDI_POS(R3, u4::new(5)),
        JMP_OFF(5),
        DECB(R3),
        JNE(3),
        LDI_POS(R3, u4::new(5)),
        ADD(R5, R1),
        ADDC(R6, R0),
        INCB(R1),
        CMP(R1, R7),
        JL(-17),
    ]; // 8795

    let program = vec![
        LDI_POS(R1, u4::new(0)),  // i
        LDI_POS(R2, u4::new(15)), // const
        LDI_IMM(R3, 1000),        // const
        // R5 acc1
        // R6 acc2
        LDI_IMM(R8, 60),
        MOV(R4, R1),
        //
        ADD(R4, R2),
        CMP(R4, R3),
        JGE(13),
        MOV(R7, R0),
        ADD(R7, R1),
        ADD(R7, R1),
        ADD(R7, R1),
        ADD(R7, R1),
        ADD(R7, R1),
        ADD(R7, R1),
        ADD(R7, R1),
        ADD(R7, R8),
        ADD(R5, R7),
        ADDC(R6, R0),
        MOV(R1, R4),
        JMP_OFF(-16),
        // DBG,
        INCB(R1),
        LDI_POS(R2, u4::new(3)),
        LDI_POS(R4, u4::new(5)),
        DECB(R2),
        JNE(7),
        ADD(R5, R1),
        ADDC(R6, R0),
        LDI_POS(R2, u4::new(3)),
        DECB(R4),
        JNE(1),
        LDI_POS(R4, u4::new(5)),
        JMP_OFF(5),
        DECB(R4),
        JNE(3),
        LDI_POS(R4, u4::new(5)),
        ADD(R5, R1),
        ADDC(R6, R0),
        INCB(R1),
        CMP(R1, R3),
        JL(-17),
    ]; // 1147

    let mut mcu = MCU::new(isa::to_bytes(&program));
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
